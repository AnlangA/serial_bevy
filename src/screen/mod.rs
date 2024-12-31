//! This example demonstrates the built-in 3d shapes in Bevy.
//! The scene includes a patterned texture and a rotation for visualizing the normals and UVs.
//!
//! You can toggle wireframes with the space bar except on wasm. Wasm does not support
//! `POLYGON_MODE_LINE` on the gpu.

use bevy::{
    prelude::*,
    window::*,
    sprite::Sprite,
    render::{camera::RenderTarget, view::RenderLayers},
};

pub struct ScreenPlugin;

impl Plugin for ScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup);
    }
}

/// 设置屏幕壁纸
fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    windows: Query<(Entity,&Monitor), With<PrimaryMonitor>>,
) {
    for (entity, monitor) in windows.iter() {
        println!("entity: {:?}", entity);
        let height = monitor.physical_height;
        let width = monitor.physical_width;
        // 创建满屏幕的矩形
        commands.spawn(Sprite {
            image: asset_server.load("壁纸.png"),
            custom_size: Some(Vec2::new(width as f32, height as f32)),
            ..default()
        });
    
        // 添加摄像机
        commands.spawn(Camera2d::default());
    }
}