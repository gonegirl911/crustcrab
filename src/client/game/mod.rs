pub mod gui;
pub mod output;
pub mod scene;

use self::{gui::Gui, output::Output, scene::Scene};
use super::{
    event_loop::{Event, EventHandler},
    renderer::Renderer,
    ClientEvent,
};
use flume::Sender;
use std::time::Duration;

pub struct Game {
    scene: Scene,
    gui: Gui,
    output: Output,
}

impl Game {
    pub fn new(renderer: &Renderer) -> Self {
        Self {
            scene: Scene::new(renderer),
            gui: Gui::new(renderer),
            output: Output::new(renderer),
        }
    }
}

impl EventHandler for Game {
    type Context<'a> = (Sender<ClientEvent>, &'a Renderer, Duration);

    fn handle(
        &mut self,
        event: &Event,
        (
            client_tx,
            renderer @ Renderer {
                surface,
                device,
                queue,
                ..
            },
            dt,
        ): Self::Context<'_>,
    ) {
        self.scene.handle(event, (client_tx, renderer, dt));
        self.output.handle(event, renderer);

        if let Event::RedrawRequested(_) = event {
            match surface.get_current_texture() {
                Ok(surface) => {
                    let mut encoder = device.create_command_encoder(&Default::default());
                    self.scene.draw(&self.output, &mut encoder);
                    self.gui.draw(&self.output, &mut encoder);
                    self.output.draw(&surface, &mut encoder);
                    queue.submit([encoder.finish()]);
                    surface.present();
                }
                Err(wgpu::SurfaceError::Lost) => renderer.recreate_surface(),
                _ => {}
            }
        }
    }
}
