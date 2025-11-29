use crate::{
    Assets,
    geometry::{self, CrossSectionVertex, Plane, Triangle, Vertex},
};

use geng::prelude::*;
use geng_utils::conversions::Vec2RealConversions;

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

pub struct State {
    geng: Geng,
    assets: Rc<Assets>,
    framebuffer_size: vec2<usize>,
    simulation_time: f32,
    next_spawn: f32,
    prefabs: Vec<Rc<ugli::VertexBuffer<Vertex>>>,
    objects: Vec<Object>,
    camera2d: Camera2d,
}

impl State {
    pub fn new(geng: Geng, assets: Rc<Assets>) -> Self {
        let prefab = |geometry| Rc::new(ugli::VertexBuffer::new_dynamic(geng.ugli(), geometry));
        Self {
            simulation_time: 0.0,
            next_spawn: 0.0,
            framebuffer_size: vec2(1, 1),
            camera2d: Camera2d {
                center: vec2::ZERO,
                rotation: Angle::ZERO,
                fov: Camera2dFov::Horizontal(17.0),
            },
            objects: Vec::new(),
            prefabs: vec![prefab(geometry::unit_cube_triangulated())],
            geng,
            assets,
        }
    }

    pub fn view(&self) -> Aabb2<f32> {
        let view = vec2(
            self.camera2d.fov.value(),
            self.camera2d.fov.value() / self.framebuffer_size.as_f32().aspect(),
        );
        Aabb2::point(self.camera2d.center).extend_symmetric(view)
    }
}

impl geng::State for State {
    fn update(&mut self, delta_time: f64) {
        let delta_time = delta_time as f32;

        self.simulation_time += delta_time;
        self.next_spawn -= delta_time;
        let mut rng = thread_rng();
        while self.next_spawn < 0.0 {
            self.next_spawn += 0.1;
            if let Some(geometry) = self.prefabs.choose(&mut rng) {
                let scale = rng.gen_range(0.3..=1.0);
                let pos_z = -scale * 2.0;

                let pos = 'outer: {
                    let mut pos = random_spawn(pos_z, self.view(), &mut rng);
                    for _ in 0..5 {
                        let mut good = true;
                        for obj in &self.objects {
                            let dist = (pos - obj.position).len();
                            if dist < (scale + obj.scale) * 1.74 {
                                // Try another one
                                pos = random_spawn(pos_z, self.view(), &mut rng);
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

        for obj in &mut self.objects {
            obj.position += vec3::UNIT_Z * 0.5 * delta_time;
            obj.rotate_y(Angle::from_degrees(45.0 * delta_time));
        }
        // Delete far objects
        self.objects.retain(|obj| obj.position.z < obj.scale * 2.0);
    }

    fn draw(&mut self, framebuffer: &mut ugli::Framebuffer) {
        self.framebuffer_size = framebuffer.size();
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

fn random_spawn(z: f32, view: Aabb2<f32>, rng: &mut impl Rng) -> vec3<f32> {
    vec3(
        rng.gen_range(view.min.x..=view.max.x),
        rng.gen_range(view.min.y..=view.max.y),
        z,
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

    // Convert coordinate system
    let mirror_x = |v: vec2<f32>| vec2(-v.x, v.y);
    let mut chain: Vec<vec2<f32>> = cross_section
        .iter()
        .map(|v| mirror_x(v.projected))
        .collect();

    // Optimize small sizes to look better
    let mut width: f32 = 0.1;
    let area = Aabb2::points_bounding_box(chain.iter().copied())
        .expect("there are at least 3 points at this moment");
    let radius = area.size() / 2.0;
    width = width.min(radius.x).min(radius.y);

    // Close the chain
    let mid = (chain[0] + chain[1]) / 2.0;
    chain.extend([chain[0], mid]);
    chain[0] = mid;

    // Draw
    geng.draw2d().draw2d(
        framebuffer,
        camera,
        &draw2d::Chain::new(Chain::new(chain), width, color, 5),
    );
}
