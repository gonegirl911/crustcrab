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
    output: Output,
    gui: Gui,
}

impl Game {
    pub fn new(renderer: &Renderer) -> Self {
        let scene = Scene::new(renderer);
        let output = Output::new(renderer);
        let gui = Gui::new(renderer, output.bind_group_layout());
        Self { output, scene, gui }
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
                    let view = surface.texture.create_view(&Default::default());
                    let mut encoder = device.create_command_encoder(&Default::default());
                    self.scene.draw(self.output.view(), &mut encoder);
                    self.output.draw(&view, &mut encoder);
                    self.gui.draw(&view, &mut encoder, self.output.bind_group());
                    queue.submit([encoder.finish()]);
                    surface.present();
                }
                Err(wgpu::SurfaceError::Lost) => renderer.recreate_surface(),
                _ => {}
            }
        }
    }
}
