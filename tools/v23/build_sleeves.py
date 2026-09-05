#!/usr/bin/env python3
"""Repair the SWAT operator's rigid sleeves without changing its bind frame.

Run Blender in background at idle priority:
  blender --background --python tools/v23/build_sleeves.py

The original dominant-bone splitter leaves long triangles crossing open arm
boundaries. Reassemble each full artist arm, weld those boundaries, bisect at
measured bone ends, cap every boundary and soften the low-poly cloth surface.
Four closed parts share one 512px image. No source assets are modified.
"""

import argparse
import json
from pathlib import Path
import struct
import sys
import time

import bmesh
import bpy
from mathutils import Matrix, Vector

ROOT = Path(__file__).resolve().parents[2]
START = time.perf_counter()
TO_ENGINE = Matrix(((1, 0, 0), (0, 0, 1), (0, -1, 0)))
NAMES = ('rig_shoulder_l', 'rig_elbow_l', 'rig_shoulder_r', 'rig_elbow_r', 'rig_spine')


def say(message):
    print(f'[sleeves {time.perf_counter()-START:.1f}s] {message}', flush=True)


def mesh_stats(mesh):
    bm = bmesh.new()
    bm.from_mesh(mesh)
    result = dict(vertices=len(bm.verts), faces=len(bm.faces),
                  boundary_edges=sum(edge.is_boundary for edge in bm.edges),
                  nonmanifold_edges=sum(not edge.is_manifold for edge in bm.edges))
    bm.free()
    return result


def select(objects):
    bpy.ops.object.select_all(action='DESELECT')
    for obj in objects:
        obj.select_set(True)
    bpy.context.view_layer.objects.active = objects[0]


def reconstructed_arm(objects, side):
    copies = []
    for name in ('shoulder', 'elbow', 'wrist'):
        obj = objects[f'rig_{name}_{side}'].copy()
        obj.data = obj.data.copy()
        bpy.context.scene.collection.objects.link(obj)
        copies.append(obj)
    select(copies)
    bpy.ops.object.join()
    obj = bpy.context.active_object
    obj.name = 'source_arm_'+side
    bm = bmesh.new()
    bm.from_mesh(obj.data)
    bmesh.ops.remove_doubles(bm, verts=list(bm.verts), dist=0.00002)
    bmesh.ops.recalc_face_normals(bm, faces=list(bm.faces))
    bm.to_mesh(obj.data)
    bm.free()
    say(f'{obj.name}: {mesh_stats(obj.data)}')
    return obj


def sleeve(source, name, start, end, material):
    obj = source.copy()
    obj.data = source.data.copy()
    obj.name = name
    obj.data.name = name
    bpy.context.scene.collection.objects.link(obj)
    axis = (end-start).normalized()
    # The independent rigid pieces must overlap when an elbow bends. Rounded,
    # closed caps are concealed inside their neighbour, not left as holes.
    first = start-axis*(0.040 if 'shoulder' in name else 0.028)
    last = end+axis*(0.030 if 'shoulder' in name else 0.010)
    bm = bmesh.new()
    bm.from_mesh(obj.data)
    for point, remove_low in ((first, True), (last, False)):
        bmesh.ops.bisect_plane(bm, geom=list(bm.verts)+list(bm.edges)+list(bm.faces),
                              dist=0.000001, plane_co=point, plane_no=axis,
                              clear_inner=remove_low, clear_outer=not remove_low)
    loose = [vertex for vertex in bm.verts if not vertex.link_faces]
    if loose:
        bmesh.ops.delete(bm, geom=loose, context='VERTS')
    bmesh.ops.remove_doubles(bm, verts=list(bm.verts), dist=0.00002)
    remaining = set(bm.verts)
    islands = []
    while remaining:
        stack = [remaining.pop()]
        island = set(stack)
        while stack:
            for edge in stack.pop().link_edges:
                for vertex in edge.verts:
                    if vertex in remaining:
                        remaining.remove(vertex)
                        island.add(vertex)
                        stack.append(vertex)
        islands.append(island)
    say(f'{name}: clipped vertex islands {sorted(map(len, islands), reverse=True)}')
    # Small disconnected scraps are armour tabs and cut finger/torso fragments,
    # not the sleeve shell. Do not turn those into floating solid shards.
    keep = max(islands, key=len)
    discard = [vertex for island in islands if island is not keep for vertex in island]
    if discard:
        bmesh.ops.delete(bm, geom=discard, context='VERTS')
    holes = [edge for edge in bm.edges if edge.is_boundary]
    caps = bmesh.ops.holes_fill(bm, edges=holes, sides=0)['faces']
    remaining_boundary = [edge for edge in bm.edges if edge.is_boundary]
    # Blender's planar hole operator may leave the authored shoulder boundary
    # (non-planar, but still a simple cycle). Seal that exact cycle explicitly.
    pending = set(remaining_boundary)
    while pending:
        first_edge = pending.pop()
        ordered = list(first_edge.verts)
        while ordered[-1] is not ordered[0]:
            edges = [edge for edge in ordered[-1].link_edges if edge in pending]
            if len(edges) != 1:
                raise RuntimeError(f'{name}: branching open boundary')
            edge = edges[0]
            pending.remove(edge)
            ordered.append(edge.other_vert(ordered[-1]))
        caps.append(bm.faces.new(ordered[:-1]))
    say(f'{name}: sealed {len(holes)} boundary edges with {len(caps)} caps')
    uv = bm.loops.layers.uv.active
    # Cap texels come from the boundary's cloth island, never the UV origin.
    for face in caps:
        samples = [loop[uv].uv.copy() for edge in face.edges for other in edge.link_faces
                   if other is not face for loop in other.loops if loop.vert in edge.verts]
        if samples:
            avg = sum(samples, Vector((0.0, 0.0)))/len(samples)
            for loop in face.loops:
                loop[uv].uv = avg
        face.material_index = 0
    bmesh.ops.recalc_face_normals(bm, faces=list(bm.faces))
    bm.to_mesh(obj.data)
    bm.free()
    # The old imported sleeve has only a few dozen large faces. Subdivision
    # rounds that authored silhouette and its folds; it does not replace it
    # with an untextured cylinder. Fixed geometry is shared by FP and TP.
    select([obj])
    modifier = obj.modifiers.new('soften_authored_cloth', 'SUBSURF')
    modifier.subdivision_type = 'CATMULL_CLARK'
    modifier.levels = 2
    bpy.ops.object.modifier_apply(modifier=modifier.name)
    for polygon in obj.data.polygons:
        polygon.use_smooth = True
        polygon.material_index = 0
    obj.data.materials.clear()
    obj.data.materials.append(material)
    stats = mesh_stats(obj.data)
    if stats['boundary_edges'] or stats['nonmanifold_edges']:
        raise RuntimeError(f'{name} is not closed: {stats}')
    say(f'{name}: {stats}, bind span {(end-start).length:.6f}m')
    return obj, stats


def cloth_material(original):
    texture = next(node.image for material in original.data.materials if material and material.use_nodes
                   for node in material.node_tree.nodes if node.type == 'TEX_IMAGE' and node.image)
    material = bpy.data.materials.new('operator_sleeve_fabric')
    material.use_nodes = True
    bsdf = material.node_tree.nodes.get('Principled BSDF')
    bsdf.inputs['Base Color'].default_value = (1, 1, 1, 1)
    bsdf.inputs['Roughness'].default_value = 0.85
    node = material.node_tree.nodes.new('ShaderNodeTexImage')
    node.image = texture
    material.node_tree.links.new(node.outputs['Color'], bsdf.inputs['Base Color'])
    return material


def repaired_torso(originals, joints, material):
    """Rebuild only shoulder borders; preserve the lower vest and its UVs."""
    copies = []
    for name in ('rig_spine', 'rig_shoulder_l', 'rig_shoulder_r'):
        obj = originals[name].copy()
        obj.data = obj.data.copy()
        bpy.context.scene.collection.objects.link(obj)
        copies.append(obj)
    select(copies)
    bpy.ops.object.join()
    upper = bpy.context.active_object
    bm = bmesh.new()
    bm.from_mesh(upper.data)
    bmesh.ops.remove_doubles(bm, verts=list(bm.verts), dist=0.00002)
    # At this height the original vest is narrower than both side cuts.
    # Split here so neither waist pouches nor bottom hem can be affected.
    seam_height = 1.18
    bmesh.ops.bisect_plane(bm, geom=list(bm.verts)+list(bm.edges)+list(bm.faces),
                          plane_co=Vector((0, 0, seam_height)), plane_no=Vector((0, 0, 1)),
                          dist=0.000001, clear_inner=True)
    left = joints['shoulder_l'].x+0.052
    right = joints['shoulder_r'].x-0.052
    for x, outer in ((left, True), (right, False)):
        bmesh.ops.bisect_plane(bm, geom=list(bm.verts)+list(bm.edges)+list(bm.faces),
                              plane_co=Vector((x, 0, 0)), plane_no=Vector((1, 0, 0)),
                              dist=0.000001, clear_outer=outer, clear_inner=not outer)
        edges = [edge for edge in bm.edges if edge.is_boundary and
                 all(abs(vertex.co.x-x) < 0.00002 for vertex in edge.verts)]
        caps = bmesh.ops.holes_fill(bm, edges=edges, sides=0)['faces']
        uv = bm.loops.layers.uv.active
        for cap in caps:
            samples = [loop[uv].uv.copy() for edge in cap.edges for face in edge.link_faces
                       if face is not cap for loop in face.loops if loop.vert in edge.verts]
            if samples:
                average = sum(samples, Vector((0, 0)))/len(samples)
                for loop in cap.loops:
                    loop[uv].uv = average
            cap.material_index = 0
        left_open = [edge for edge in bm.edges if edge.is_boundary and
                     all(abs(vertex.co.x-x) < 0.00002 for vertex in edge.verts)]
        if left_open:
            raise RuntimeError(f'torso shoulder x={x}: {len(left_open)} edges remain open')
        say(f'torso shoulder x={x:.4f}: {len(edges)} boundary edges sealed with {len(caps)} caps')
    loose = [vertex for vertex in bm.verts if not vertex.link_faces]
    if loose:
        bmesh.ops.delete(bm, geom=loose, context='VERTS')
    bmesh.ops.recalc_face_normals(bm, faces=list(bm.faces))
    bm.to_mesh(upper.data)
    bm.free()
    lower = originals['rig_spine'].copy()
    lower.data = lower.data.copy()
    bpy.context.scene.collection.objects.link(lower)
    bm = bmesh.new()
    bm.from_mesh(lower.data)
    bmesh.ops.bisect_plane(bm, geom=list(bm.verts)+list(bm.edges)+list(bm.faces),
                          plane_co=Vector((0, 0, seam_height)), plane_no=Vector((0, 0, 1)),
                          dist=0.000001, clear_outer=True)
    bm.to_mesh(lower.data)
    bm.free()
    select([upper, lower])
    bpy.ops.object.join()
    torso = bpy.context.active_object
    originals['rig_spine'].name = 'old_rig_spine'
    torso.name = 'rig_spine'
    torso.data.name = 'sealed_torso_shoulders'
    bm = bmesh.new()
    bm.from_mesh(torso.data)
    bmesh.ops.remove_doubles(bm, verts=list(bm.verts), dist=0.00002)
    bm.to_mesh(torso.data)
    bm.free()
    torso.data.materials.clear()
    torso.data.materials.append(material)
    for face in torso.data.polygons:
        face.material_index = 0
    say(f'torso repair keeps original shape/UVs below {seam_height}m, no subdivision')
    return torso


def bake_sleeve_atlas(objects, directory):
    """Spend the 512px image on sleeve fabric, not the unused full-body atlas."""
    copies, counts = [], []
    for obj in objects:
        copy = obj.copy()
        copy.data = obj.data.copy()
        bpy.context.scene.collection.objects.link(copy)
        copies.append(copy)
        counts.append(len(copy.data.loops))
    select(copies)
    bpy.ops.object.join()
    joined = bpy.context.active_object
    source = joined.data.uv_layers[0]
    source.name = 'SourceUV'
    values = [tuple(loop.uv) for loop in source.data]
    atlas_uv = joined.data.uv_layers.new(name='AtlasUV')
    for loop, value in zip(atlas_uv.data, values):
        loop.uv = value
    joined.data.uv_layers.active_index = 1
    joined.data.uv_layers['AtlasUV'].active_render = True
    bpy.ops.object.mode_set(mode='EDIT')
    bpy.ops.mesh.select_all(action='SELECT')
    bpy.ops.uv.select_all(action='SELECT')
    bpy.ops.uv.pack_islands(margin_method='ADD', margin=0.008, rotate=True)
    bpy.ops.object.mode_set(mode='OBJECT')
    material = joined.data.materials[0].copy()
    joined.data.materials.clear()
    joined.data.materials.append(material)
    for face in joined.data.polygons:
        face.material_index = 0
    nodes, links = material.node_tree.nodes, material.node_tree.links
    src = next(node for node in nodes if node.type == 'TEX_IMAGE')
    source_uv = nodes.new('ShaderNodeUVMap')
    source_uv.uv_map = 'SourceUV'
    links.new(source_uv.outputs['UV'], src.inputs['Vector'])
    atlas = bpy.data.images.new('operator_sleeves_512', width=512, height=512, alpha=False)
    target = nodes.new('ShaderNodeTexImage')
    target.image = atlas
    for node in nodes:
        node.select = False
    target.select = True
    nodes.active = target
    scene = bpy.context.scene
    scene.render.engine = 'CYCLES'
    scene.cycles.device = 'CPU'
    scene.cycles.samples = 8
    scene.render.bake.use_pass_direct = False
    scene.render.bake.use_pass_indirect = False
    scene.render.bake.use_pass_color = True
    scene.render.bake.margin = 4
    scene.render.bake.use_selected_to_active = False
    select([joined])
    bpy.ops.object.bake(type='DIFFUSE')
    atlas.file_format = 'PNG'
    atlas.filepath_raw = str(directory/'operator-sleeves-512.png')
    atlas.save()
    atlas.pack()
    packed = [tuple(loop.uv) for loop in joined.data.uv_layers['AtlasUV'].data]
    final = bpy.data.materials.new('sealed_operator_sleeves')
    final.use_nodes = True
    bsdf = final.node_tree.nodes.get('Principled BSDF')
    bsdf.inputs['Base Color'].default_value = (1, 1, 1, 1)
    bsdf.inputs['Roughness'].default_value = 0.85
    image = final.node_tree.nodes.new('ShaderNodeTexImage')
    image.image = atlas
    final.node_tree.links.new(image.outputs['Color'], bsdf.inputs['Base Color'])
    offset = 0
    for obj, count in zip(objects, counts):
        for loop, value in zip(obj.data.uv_layers[0].data, packed[offset:offset+count]):
            loop.uv = value
        offset += count
        obj.data.materials.clear()
        obj.data.materials.append(final)
    bpy.data.objects.remove(joined, do_unlink=True)
    say('baked sleeve-only atlas')


def preview(objects, directory):
    scene = bpy.context.scene
    scene.render.engine = 'BLENDER_WORKBENCH'
    scene.display.shading.light = 'STUDIO'
    scene.display.shading.color_type = 'TEXTURE'
    scene.display.shading.show_shadows = True
    scene.display.shading.show_cavity = True
    scene.display.shading.cavity_type = 'BOTH'
    scene.display.shading.background_type = 'WORLD'
    if scene.world is None:
        scene.world = bpy.data.worlds.new('sleeve-inspection-world')
    scene.world.color = (0.16, 0.18, 0.22)
    scene.render.resolution_x = 1280
    scene.render.resolution_y = 800
    scene.render.resolution_percentage = 100
    scene.render.image_settings.file_format = 'PNG'
    for obj in bpy.data.objects:
        obj.hide_render = True
    for obj in objects:
        obj.hide_render = False
    data = bpy.data.cameras.new('sleeve-inspection')
    camera = bpy.data.objects.new('sleeve-inspection', data)
    scene.collection.objects.link(camera)
    scene.camera = camera
    target = Vector((0, 0, 1.35))
    for name, offset in [('front', (0, -1.65, 0.2)), ('ends', (1.2, -1.0, 0.6))]:
        camera.location = target+Vector(offset)
        camera.rotation_euler = (target-camera.location).to_track_quat('-Z', 'Y').to_euler()
        data.type = 'ORTHO'
        data.ortho_scale = 1.65
        scene.render.filepath = str(directory/f'sleeves-{name}.png')
        bpy.ops.render.render(write_still=True)
    bpy.data.objects.remove(camera, do_unlink=True)


def verify_glb(path):
    raw = path.read_bytes()
    size = struct.unpack_from('<I', raw, 12)[0]
    doc = json.loads(raw[20:20+size])
    binary_start = 20+size+8
    names = sorted(node['name'] for node in doc['nodes'] if 'mesh' in node)
    assert names == sorted(NAMES), names
    assert len(doc['images']) == 2, 'one sleeve atlas and original torso texture only'
    dimensions = []
    for image in doc['images']:
        view = doc['bufferViews'][image['bufferView']]
        png = raw[binary_start+view.get('byteOffset', 0):binary_start+view.get('byteOffset', 0)+view['byteLength']]
        assert png[:8] == b'\x89PNG\r\n\x1a\n'
        dimensions.append(struct.unpack_from('>IIB', png, 16))
    assert sorted(dimensions) == [(512, 512, 8), (1024, 1024, 8)], dimensions
    triangles = 0
    for mesh in doc['meshes']:
        assert len(mesh['primitives']) == 1
        primitive = mesh['primitives'][0]
        assert {'POSITION', 'NORMAL', 'TEXCOORD_0'} <= primitive['attributes'].keys()
        material = doc['materials'][primitive['material']]['pbrMetallicRoughness']
        assert 'baseColorTexture' in material
        assert material.get('baseColorFactor', [1, 1, 1, 1]) == [1, 1, 1, 1]
        triangles += doc['accessors'][primitive['indices']]['count']//3
    say(f'export verified: {len(raw):,} bytes, {triangles:,} triangles, 512px sleeve + 1024px torso 8-bit PNGs')
    return dict(bytes=len(raw), triangles=triangles, images=dimensions, names=names)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', type=Path, default=ROOT/'crates/arena/assets/weapon-sleeves.glb')
    parser.add_argument('--preview-dir', type=Path, default=ROOT/'target/sleeve-previews')
    parser.add_argument('--inspect-torso', action='store_true', help='Render the original torso only; do not write an asset')
    options = parser.parse_args(sys.argv[sys.argv.index('--')+1:] if '--' in sys.argv else [])
    if not options.output.is_absolute():
        options.output = ROOT/options.output
    if not options.preview_dir.is_absolute():
        options.preview_dir = ROOT/options.preview_dir
    options.preview_dir.mkdir(parents=True, exist_ok=True)
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=str(ROOT/'assets/models/swat-parts.glb'))
    originals = {obj.name: obj for obj in bpy.data.objects if obj.type == 'MESH'}
    for obj in originals.values():
        world = obj.matrix_world.copy()
        obj.parent = None
        obj.matrix_world = Matrix.Identity(4)
        obj.data.transform(world)
        obj.modifiers.clear()
    if options.inspect_torso:
        say(f'original torso: {mesh_stats(originals["rig_spine"].data)}')
        for low, high in ((1.10, 1.18), (1.18, 1.24), (1.24, 1.32), (1.32, 1.40), (1.40, 1.50)):
            positions = [vertex.co for vertex in originals['rig_spine'].data.vertices if low <= vertex.co.z <= high]
            say(f'torso z {low}..{high}: x {min(p.x for p in positions):.3f}..{max(p.x for p in positions):.3f}')
        preview([originals['rig_spine']], options.preview_dir)
        return
    joints = {name: TO_ENGINE.transposed() @ Vector(value) for name, value in
              json.loads((ROOT/'assets/models/swat-rig.json').read_text())['joints'].items()}
    material = cloth_material(originals['rig_elbow_l'])
    made, stats = [], {}
    for side in ('l', 'r'):
        source = reconstructed_arm(originals, side)
        for joint, child in (('shoulder', 'elbow'), ('elbow', 'wrist')):
            name = f'rig_{joint}_{side}'
            # Release source names so the exact runtime contract exports.
            originals[name].name = 'old_'+name
            obj, stats[name] = sleeve(source, name, joints[f'{joint}_{side}'], joints[f'{child}_{side}'], material)
            made.append(obj)
    bake_sleeve_atlas(made, options.preview_dir)
    torso = repaired_torso(originals, joints, material)
    made.append(torso)
    stats['rig_spine'] = dict(shoulder_cuts_closed=True, lower_vest_preserved=True,
                             **mesh_stats(torso.data))
    select(made)
    bpy.ops.export_scene.gltf(filepath=str(options.output), export_format='GLB', export_yup=True,
                              use_selection=True, export_animations=False)
    exported = verify_glb(options.output)
    preview(made, options.preview_dir)
    (options.preview_dir/'report.json').write_text(json.dumps(dict(parts=stats, exported=exported,
        wall_seconds=round(time.perf_counter()-START, 2)), indent=2)+'\n', encoding='utf-8')
    say('finished')


if __name__ == '__main__':
    main()
