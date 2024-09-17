mod geometry;
mod state;

use self::state::State;

use geng::prelude::*;

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
