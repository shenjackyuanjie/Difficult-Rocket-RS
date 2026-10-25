use crate::sprite_shader::*;
use bevy::{
    app::TaskPoolPlugin,
    asset::{AssetServer, io::ErasedAssetReader},
    diagnostic::DiagnosticsPlugin,
    shader::{Shader, ShaderImport, ShaderRef},
    sprite_render::{Material2d, SpriteMeshMaterial, SpriteMeshMaterialPlugin},
    tasks::block_on,
};
use naga::{
    back::hlsl::{BindTarget, Options, PipelineOptions, Writer},
    valid::{Capabilities, ValidationFlags, Validator},
};

const SHADER_URI: &str = "embedded://bevy_sprite_render/sprite_mesh/sprite_material.wesl";

fn official_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        TaskPoolPlugin::default(),
        AssetPlugin {
            watch_for_changes_override: Some(false),
            use_asset_processor_override: Some(false),
            ..default()
        },
        DiagnosticsPlugin,
        SpriteMeshMaterialPlugin,
    ));
    app
}

fn read_source(reader: &dyn ErasedAssetReader) -> String {
    block_on(async {
        let mut file = reader.read(Path::new(SHADER_KEY)).await.unwrap();
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).await.unwrap();
        String::from_utf8(bytes).unwrap()
    })
}

fn official_source() -> String {
    let app = official_app();
    let server = app.world().resource::<AssetServer>();
    read_source(server.get_source("embedded").unwrap().reader())
}

#[test]
fn plugin_replaces_official_asset_for_an_existing_reader() {
    let mut app = official_app();
    let server = app.world().resource::<AssetServer>().clone();
    let reader = server.get_source("embedded").unwrap().reader();
    let original = read_source(reader);
    let expected = patch_source(&original);

    app.add_plugins(SpriteShaderPlugin);

    assert_ne!(original, expected);
    assert_eq!(read_source(reader).as_bytes(), expected.as_bytes());
    let registered = app
        .world()
        .resource::<EmbeddedAssetRegistry>()
        .remove_asset(Path::new(SHADER_KEY))
        .unwrap();
    assert_eq!(registered.value(), expected.as_bytes());
}

#[test]
fn patch_changes_exactly_five_locations_and_preserves_all_other_bytes() {
    let original = official_source();
    let patched = patch_source(&original);
    let mut expected: Vec<_> = original.lines().map(str::to_owned).collect();
    assert_eq!(expected.len(), 86);
    assert_eq!(
        expected[21],
        "    let instance_index = vertex.instance_index;"
    );
    expected[24] =
        "    let slot = mesh2d_bindings::mesh[vertex.instance_index].material_bind_group_slot;"
            .into();
    expected[35] =
        "    let uncompressed_vertex = decompress_vertex(vertex, vertex.instance_index);".into();
    expected[40] =
        "    var world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);"
            .into();
    expected[64] = "    out.instance_index = vertex.instance_index;".into();
    expected.remove(21);
    let expected = expected.join("\n") + "\n";
    assert_eq!(patched.as_bytes(), expected.as_bytes());
    assert_eq!(
        patched.split_once("@fragment").unwrap().1,
        original.split_once("@fragment").unwrap().1
    );
    assert!(patched.contains("uncompressed_vertex.normal, vertex.instance_index)"));
}

#[test]
fn patch_requires_exactly_one_match_for_each_edit() {
    let original = official_source();
    for (before, _) in EDITS {
        let missing = original.replace(before, "");
        assert!(std::panic::catch_unwind(|| patch_source(&missing)).is_err());
        let duplicated = format!("{original}{before}");
        assert!(std::panic::catch_unwind(|| patch_source(&duplicated)).is_err());
    }
}

#[test]
fn original_shader_refs_package_and_imports_remain_unchanged() {
    for shader_ref in [
        SpriteMeshMaterial::vertex_shader(),
        SpriteMeshMaterial::fragment_shader(),
    ] {
        let ShaderRef::Path(path) = shader_ref else {
            panic!("官方 Sprite shader 应继续使用原 embedded AssetPath");
        };
        assert_eq!(path, bevy::asset::AssetPath::parse(SHADER_URI));
    }
    let original = official_source();
    let patched = patch_source(&original);
    let original = Shader::from_wesl(original, SHADER_URI);
    let patched = Shader::from_wesl(patched, SHADER_URI);
    assert_eq!(patched.path, original.path);
    assert_eq!(patched.import_path, original.import_path);
    assert_eq!(patched.imports, original.imports);
    assert_eq!(
        patched.import_path,
        ShaderImport::Custom("bevy_sprite_render::sprite_mesh::sprite_material".into())
    );
    assert!(!patched.imports.is_empty());
}

fn hlsl_for(body: &str) -> String {
    let source = format!(
        "struct Mesh {{ position: vec4<f32>, }};
             struct Vertex {{ @builtin(instance_index) instance_index: u32, }};
             @group(1) @binding(0) var<storage, read> mesh: array<Mesh>;
             @vertex fn vertex(vertex: Vertex) -> @builtin(position) vec4<f32> {{
    {body}
}}
"
    );
    let module = naga::front::wgsl::parse_str(&source).unwrap();
    let info = Validator::new(ValidationFlags::all(), Capabilities::all())
        .validate(&module)
        .unwrap();
    let mut options = Options {
        special_constants_binding: Some(BindTarget {
            register: 9,
            ..default()
        }),
        fake_missing_bindings: false,
        ..default()
    };
    options.binding_map.insert(
        naga::ResourceBinding {
            group: 1,
            binding: 0,
        },
        BindTarget {
            space: 1,
            register: 0,
            ..default()
        },
    );
    let pipeline = PipelineOptions {
        entry_point: Some((naga::ShaderStage::Vertex, "vertex".to_owned())),
    };
    let mut hlsl = String::new();
    let reflection = Writer::new(&mut hlsl, &options, &pipeline)
        .write(&module, &info, None)
        .unwrap();
    assert!(reflection.entry_point_names.iter().all(Result::is_ok));
    hlsl
}

#[test]
fn naga_30_0_1_named_builtin_reproduces_double_first_instance_compensation() {
    let named = hlsl_for(
        "let i = vertex.instance_index;
    return mesh[i].position;",
    );
    let direct = hlsl_for("return mesh[vertex.instance_index].position;");
    assert_eq!(named.matches(".first_instance").count(), 2);
    assert_eq!(direct.matches(".first_instance").count(), 1);
    assert!(named.contains("uint i = (_NagaConstants.first_instance + vertex_1.instance_index);"));
    assert!(named.contains("(_NagaConstants.first_instance + i)*16"));
    assert!(direct.contains("(_NagaConstants.first_instance + vertex_1.instance_index)*16"));
    assert!(!direct.contains("uint i ="));
}
