mod camera3d;
mod geometry;

use self::{camera3d::Camera3d, geometry::Plane};

use geng::prelude::*;
use geometry::{CrossSectionVertex, Triangle, Vertex};

#[derive(clap::Parser)]
struct Opts {
    #[clap(flatten)]
    window: geng::CliArgs,
}

#[derive(geng::asset::Load)]
pub struct Assets {
    pub config: Config,
}

#[derive(geng::asset::Load, Deserialize)]
#[load(serde = "ron")]
pub struct Config {
    background_color: Rgba<f32>,
    object_colors: Vec<Rgba<f32>>,
}

fn main() {
    logger::init();

    let opts: Opts = clap::Parser::parse();

    let mut context = geng::ContextOptions::default();
    context.with_cli(&opts.window);
    Geng::run_with(&context, |geng| async move {
        let manager = geng.asset_manager();
        let assets: Rc<Assets> = geng::asset::Load::load(manager, &run_dir().join("assets"), &())
            .await
            .expect("failed to load assets");
        geng.run_state(State::new(geng.clone(), assets)).await
    })
}

pub struct Object {
    pub geometry: Rc<ugli::VertexBuffer<Vertex>>,
    pub position: vec3<f32>,
    pub orientation: vec3<f32>,
    pub roll: Angle<f32>,
    pub scale: f32,
    pub color: Rgba<f32>,
}

impl Object {
    pub fn new(position: vec3<f32>, geometry: Rc<ugli::VertexBuffer<Vertex>>) -> Self {
        Self {
            geometry,
            position,
            orientation: vec3::UNIT_X,
            roll: Angle::ZERO,
            scale: 1.0,
            color: Rgba::WHITE,
        }
    }

    pub fn matrix(&self) -> mat4<f32> {
        let flat = vec2(self.orientation.x, self.orientation.z);
        let rot_h = flat.arg();
        let rot_v = vec2(flat.len(), self.orientation.y).arg();
        mat4::translate(self.position)
            * mat4::rotate_x(self.roll)
            * mat4::rotate_z(-rot_v)
            * mat4::rotate_y(rot_h)
            * mat4::scale_uniform(self.scale)
    }

    pub fn rotate_y(&mut self, angle: Angle<f32>) {
        let flat = vec2(self.orientation.x, self.orientation.z);
        let flat = flat.rotate(angle);
        self.orientation = vec3(flat.x, self.orientation.y, flat.y);
    }
}

struct State {
    geng: Geng,
    assets: Rc<Assets>,
    simulation_time: f32,
    next_spawn: f32,
    prefabs: Vec<Rc<ugli::VertexBuffer<Vertex>>>,
    objects: Vec<Object>,
    camera3d: Camera3d,
    camera2d: Camera2d,
    paused: bool,
}

impl State {
    pub fn new(geng: Geng, assets: Rc<Assets>) -> Self {
        let prefab = |geometry| Rc::new(ugli::VertexBuffer::new_dynamic(geng.ugli(), geometry));
        Self {
            simulation_time: 0.0,
            next_spawn: 0.0,
            camera3d: Camera3d {
                fov: Angle::from_radians(70.0),
                pos: vec3(6.0, 0.0, 10.0),
                rot_h: Angle::from_degrees(30.0),
                rot_v: Angle::ZERO,
            },
            camera2d: Camera2d {
                center: vec2::ZERO,
                rotation: Angle::ZERO,
                fov: 10.0,
            },
            objects: Vec::new(),
            prefabs: vec![prefab(geometry::unit_cube_triangulated())],
            paused: false,
            geng,
            assets,
        }
    }
}

impl geng::State for State {
    fn handle_event(&mut self, event: geng::Event) {
        if geng_utils::key::is_event_press(&event, [geng::Key::P]) {
            self.paused = !self.paused;
        }
    }

    fn update(&mut self, delta_time: f64) {
        let delta_time = delta_time as f32;

        if !self.paused {
            self.simulation_time += delta_time;
            self.next_spawn -= delta_time;
            let mut rng = thread_rng();
            while self.next_spawn < 0.0 {
                // Final range: 0.1..=0.5
                self.next_spawn += rng.gen_range(0.0..=1.0).sqr() * 0.4 + 0.1;
                if let Some(geometry) = self.prefabs.choose(&mut rng) {
                    let scale = rng.gen_range(0.3..=1.0);
                    let pos = 'outer: {
                        let mut pos = random_spawn(&mut rng);
                        for _ in 0..5 {
                            let mut good = true;
                            for obj in &self.objects {
                                let dist = (pos - obj.position).len();
                                if dist < (scale + obj.scale) * 1.74 {
                                    // Try another one
                                    pos = random_spawn(&mut rng);
                                    good = false;
                                    break;
                                }
                            }
                            if good {
                                break 'outer Some(pos);
                            }
                        }
                        None
                    };

                    if let Some(pos) = pos {
                        let mut obj = Object::new(pos, geometry.clone());
                        obj.orientation = vec3(
                            rng.gen_range(-1.0..=1.0),
                            rng.gen_range(-1.0..=1.0),
                            rng.gen_range(-1.0..=1.0),
                        );
                        obj.roll = Angle::from_degrees(rng.gen_range(0.0..=360.0));
                        obj.scale = scale;
                        obj.color = self
                            .assets
                            .config
                            .object_colors
                            .choose(&mut rng)
                            .copied()
                            .unwrap_or(Rgba::WHITE);
                        self.objects.push(obj);
                    }
                }
            }
        }

        let mut move_dir_x = 0.0;
        let mut move_dir_z = 0.0;

        if self.geng.window().is_key_pressed(geng::Key::A) {
            move_dir_x -= 1.0;
        }
        if self.geng.window().is_key_pressed(geng::Key::D) {
            move_dir_x += 1.0;
        }
        if self.geng.window().is_key_pressed(geng::Key::S) {
            move_dir_z -= 1.0;
        }
        if self.geng.window().is_key_pressed(geng::Key::W) {
            move_dir_z += 1.0;
        }

        let mut look_dir = self.camera3d.look_dir();
        look_dir.y = 0.0;

        let side_dir = vec2(look_dir.x, look_dir.z);
        let side_dir = side_dir.rotate_90();
        let side_dir = vec3(side_dir.x, 0.0, side_dir.y);

        let mut move_dir_y = 0.0;
        if self.geng.window().is_key_pressed(geng::Key::Space) {
            move_dir_y += 1.0;
        }
        if self.geng.window().is_key_pressed(geng::Key::ShiftLeft) {
            move_dir_y -= 1.0;
        }

        let move_dir = look_dir * move_dir_z + side_dir * move_dir_x + vec3::UNIT_Y * move_dir_y;

        self.camera3d.pos += move_dir * 5.0 * delta_time;

        let mut rotate = 0.0;
        if self.geng.window().is_key_pressed(geng::Key::Q) {
            rotate += 1.0;
        }
        if self.geng.window().is_key_pressed(geng::Key::E) {
            rotate -= 1.0;
        }
        self.camera3d.rot_h += Angle::from_degrees(rotate * 180.0 * delta_time);

        if !self.paused {
            for obj in &mut self.objects {
                obj.position += vec3::UNIT_Z * 0.5 * delta_time;
                obj.rotate_y(Angle::from_degrees(45.0 * delta_time));
            }
            // Delete far objects
            self.objects.retain(|obj| obj.position.z < 5.0);
        }
    }

    fn draw(&mut self, framebuffer: &mut ugli::Framebuffer) {
        ugli::clear(
            framebuffer,
            Some(self.assets.config.background_color),
            None,
            None,
        );

        let cross_plane = Plane {
            normal: vec3(0.0, 0.0, 1.0),
            offset: 0.0,
        };

        // Calculate a cross section
        let cross_sections: Vec<(usize, Vec<CrossSectionVertex>)> = self
            .objects
            .iter()
            .enumerate()
            .flat_map(|(i, obj)| {
                let a = obj.geometry.iter().step_by(3);
                let b = obj.geometry.iter().skip(1).step_by(3);
                let c = obj.geometry.iter().skip(2).step_by(3);
                let transform = |v: vec3<f32>| (obj.matrix() * v.extend(1.0)).into_3d();
                let triangles = itertools::izip![a, b, c].map(|(a, b, c)| {
                    Triangle::new(transform(a.a_pos), transform(b.a_pos), transform(c.a_pos))
                });
                let cross_section = cross_plane.cross_sect(triangles);
                (cross_section.len() >= 3).then_some((i, cross_section))
            })
            .collect();

        // Draw the cross section in 2d
        for (i, cross_section) in &cross_sections {
            let i = *i;
            draw_flat_section(
                cross_section,
                self.objects[i].color,
                &self.camera2d,
                &self.geng,
                framebuffer,
            );
        }
    }
}

fn random_spawn(rng: &mut impl Rng) -> vec3<f32> {
    let radius = 7.0;
    vec3(
        rng.gen_range(-radius..=radius),
        rng.gen_range(-radius..=radius),
        -5.0,
    )
}

fn draw_flat_section(
    cross_section: &[CrossSectionVertex],
    color: Rgba<f32>,
    camera: &Camera2d,
    geng: &Geng,
    framebuffer: &mut ugli::Framebuffer,
) {
    if cross_section.len() < 3 {
        return;
    }

    let mirror_x = |v: vec2<f32>| vec2(-v.x, v.y);
    let mut chain: Vec<vec2<f32>> = cross_section
        .iter()
        .map(|v| mirror_x(v.projected))
        .collect();
    let mid = (chain[0] + chain[1]) / 2.0;
    chain.extend([chain[0], mid]);
    chain[0] = mid;
    geng.draw2d().draw2d(
        framebuffer,
        camera,
        &draw2d::Chain::new(Chain::new(chain), 0.1, color, 5),
    );
}
