//! This example demonstrates the built-in 3d shapes in Bevy.
//! The scene includes a patterned texture and a rotation for visualizing the normals and UVs.
//!
//! You can toggle wireframes with the space bar except on wasm. Wasm does not support
//! `POLYGON_MODE_LINE` on the gpu.

use bevy::{prelude::*, render::camera::RenderTarget, sprite::Sprite, window::*};

pub struct ScreenPlugin;

impl Plugin for ScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup);
    }
}

/// set wallpaper
fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    monitor_windows: Query<(Entity, &Monitor), With<PrimaryMonitor>>,
    primary_window: Query<Entity, With<PrimaryWindow>>,
) {
    // Monitor Entity is different from Window Entity
    let primary_window_entity = primary_window.get_single().unwrap();
    info!("primary_window_entity: {:?}", primary_window_entity);

    let (entity, monitor) = monitor_windows.get_single().unwrap();
    info!("entity: {:?}", entity);
    let height = monitor.physical_height;
    let width = monitor.physical_width;
    // create a full screen rectangle
    commands.spawn(Sprite {
        image: asset_server.load("壁纸.png"),
        custom_size: Some(Vec2::new(width as f32, height as f32)),
        ..default()
    });

    // add camera
    commands.spawn((Camera2d::default(), Camera {
        target: RenderTarget::Window(WindowRef::Entity(primary_window_entity)),
        ..Default::default()
    }));
}
