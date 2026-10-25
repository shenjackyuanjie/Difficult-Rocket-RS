//! 在官方 Sprite shader 首次加载前展开 instance_index 别名，规避 Naga 30.0.1 的 HLSL 重复补偿。
use bevy::{asset::io::embedded::EmbeddedAssetRegistry, prelude::*};
use std::path::{Path, PathBuf};

const SHADER_KEY: &str = "bevy_sprite_render/sprite_mesh/sprite_material.wesl";
const EDITS: [(&str, &str); 5] = [
    ("    let instance_index = vertex.instance_index;\n", ""),
    ("mesh[instance_index]", "mesh[vertex.instance_index]"),
    (
        "decompress_vertex(vertex, instance_index)",
        "decompress_vertex(vertex, vertex.instance_index)",
    ),
    (
        "get_world_from_local(instance_index)",
        "get_world_from_local(vertex.instance_index)",
    ),
    (
        "out.instance_index = instance_index;",
        "out.instance_index = vertex.instance_index;",
    ),
];

pub struct SpriteShaderPlugin;

impl Plugin for SpriteShaderPlugin {
    fn build(&self, app: &mut App) {
        // DefaultPlugins 已注册原始 bytes；Material2dPlugin 到 RenderStartup 才首次加载它。
        let registry = app.world().resource::<EmbeddedAssetRegistry>();
        let original = registry
            .remove_asset(Path::new(SHADER_KEY))
            .expect("DefaultPlugins 应已注册官方 Sprite shader");
        let source = std::str::from_utf8(original.value()).expect("官方 Sprite shader 应为 UTF-8");
        let patched = patch_source(source);
        registry.insert_asset(
            PathBuf::from(SHADER_KEY),
            Path::new(SHADER_KEY),
            patched.into_bytes(),
        );
        info!("已展开官方 Sprite shader 的 instance_index 别名，保留原 embedded 路径与 Material");
    }
}

fn patch_source(source: &str) -> String {
    let mut patched = source.to_owned();
    for (before, after) in EDITS {
        assert_eq!(
            patched.matches(before).count(),
            1,
            "Bevy 0.20 Sprite shader 应恰好包含一处 {before:?}"
        );
        patched = patched.replace(before, after);
    }
    patched
}

#[cfg(test)]
mod tests;
