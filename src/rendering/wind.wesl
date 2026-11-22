// Wind vertex shader for grass, tree canopies and flowers (WESL).
//
// Same as Bevy's standard mesh vertex shader (without skinning and morph
// targets), plus a sway applied in world space. How much a vertex sways
// comes from its vertex-colour alpha (0 = rooted, 1 = free tip); opaque
// materials ignore alpha, so it is free to carry this. The phase comes from
// world position, so gusts visibly roll across the meadow.
// Parameters: x = amplitude (m), y = speed, z = flutter, w = breeze
// strength (weather; 0 = unset).

import bevy_pbr::render::{
    mesh_functions,
    forward_io::{Vertex, VertexOutput, decompress_vertex},
    view_transformations::position_world_to_clip,
    mesh_view_bindings::globals,
};

struct Wind {
    params: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> wind: Wind;

@vertex
fn vertex(vertex_in: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let vertex = decompress_vertex(vertex_in, vertex_in.instance_index);
    let world_from_local = mesh_functions::get_world_from_local(vertex_in.instance_index);
    var world = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));

    var weight = 0.0;
    @if(VERTEX_COLORS) {
        weight = vertex.color.a;
    }

    let t = globals.time;
    let p = world.xz;
    let breeze = select(1.0, wind.params.w, wind.params.w > 0.0);
    let amplitude = wind.params.x * breeze;
    let speed = wind.params.y;
    let gust = sin(t * 0.4 + p.x * 0.045 + p.y * 0.03) * 0.5 + 0.5;
    let wave = sin(t * speed + p.x * 0.35 + p.y * 0.22);
    let flutter = sin(t * speed * 2.7 + p.x * 1.3 + p.y * 1.7) * wind.params.z;
    let s = (wave * (0.35 + 0.9 * gust) + flutter) * amplitude * weight;
    world.x += s * 0.85;
    world.z += s * 0.45;
    world.y -= abs(s) * 0.3 * weight;

    out.world_position = world;
    out.position = position_world_to_clip(world.xyz);
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex_in.instance_index);

    @if(VERTEX_UVS_A)
    out.uv = vertex.uv;
    @if(VERTEX_COLORS)
    out.color = vec4<f32>(vertex.color.rgb, 1.0);
    @if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = vertex_in.instance_index;
    @if(VISIBILITY_RANGE_DITHER)
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        vertex_in.instance_index, world_from_local[3]);

    return out;
}
