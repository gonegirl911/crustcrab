pub mod output;
pub mod player;
pub mod scene;

use self::{output::Output, player::Player, scene::Scene};
use super::{
    event_loop::{Event, EventHandler},
    renderer::Renderer,
    ClientEvent,
};
use flume::Sender;
use std::time::Duration;

pub struct Game {
    player: Player,
    scene: Scene,
    output: Output,
}

impl Game {
    pub fn new(renderer: &Renderer) -> Self {
        let output = Output::new(renderer);
        let player = Player::new(renderer, output.bind_group_layout());
        let scene = Scene::new(renderer, player.bind_group_layout());
        Self {
            player,
            scene,
            output,
        }
    }

    fn draw(&self, view: &wgpu::TextureView, encoder: &mut wgpu::CommandEncoder) {
        self.scene.draw(
            self.output.view(),
            encoder,
            self.player.bind_group(),
            &self.player.frustum(),
        );
        self.output.draw(view, encoder);
        self.player.draw(view, encoder, self.output.bind_group());
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
        self.player.handle(event, (client_tx, renderer, dt));
        self.scene.handle(event, renderer);
        self.output.handle(event, renderer);

        if let Event::RedrawRequested(_) = event {
            match surface.get_current_texture() {
                Ok(surface) => {
                    let view = surface.texture.create_view(&Default::default());
                    let mut encoder = device.create_command_encoder(&Default::default());
                    self.draw(&view, &mut encoder);
                    queue.submit([encoder.finish()]);
                    surface.present();
                }
                Err(wgpu::SurfaceError::Lost) => renderer.recreate_surface(),
                _ => {}
            }
        }
    }
}
