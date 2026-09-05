#!/usr/bin/env python3
"""Bake the operator's glove rig into weapon-specific first-person grips.

Blender --background --python tools/v23/build_grips.py -- --source-root /artist/assets

The existing weapon GLB remains unchanged. Each output hand is in its weapon's
local frame; wrist sockets use engine +X forward, +Y up, +Z right. The source
rig is posed before its skin is evaluated: fingers are never translated as
detached rigid pieces. Previews and audit logs stay under target/.
"""

import argparse
from collections import Counter
import importlib.util
import json
import math
from pathlib import Path
import struct
import sys
import time

import bpy
import bmesh
from mathutils import Euler, Matrix, Vector
from mathutils.bvhtree import BVHTree

REPO = Path(__file__).resolve().parents[2]
START = time.perf_counter()
ATLAS_SIZE = 512
AXIS_TO_ENGINE = Matrix(((1, 0, 0), (0, 0, 1), (0, -1, 0)))
FINGERS = ('Index', 'Middle', 'Ring', 'Pinky', 'Thumb')
SOURCE_GRIPS = {'Right': Vector((-0.045, 0.0, -0.075)), 'Left': Vector((0.235, 0.0, -0.130))}

# Measured in the shipped weapons, Blender coordinates (+X forward, +Z up).
# Grip centres anchor the palm; trigger targets address the distinct trigger
# locations rather than treating the nominal file origin as a trigger socket.
PROFILES = {
    1: dict(name='sidearm', weapon='rifle', right=(-0.045, 0, -0.075), right_euler=(0, 0, 0),
            left=(0.235, 0, -0.130), left_euler=(0, 0, 0), trigger=(0.008, -0.009, -0.028)),
    2: dict(name='vityaz', weapon='w_vityaz', right=(-0.048, 0, -0.065), right_euler=(0, 12, 0),
            left=(0.385, 0, 0.052), left_euler=(-90, 0, 0), trigger=(0.020, -0.014, -0.021)),
    3: dict(name='ak47', weapon='w_ak47', right=(-0.053, 0, -0.032), right_euler=(0, 16, 0),
            left=(0.350, 0, 0.061), left_euler=(-90, 0, 0), trigger=(0.055, -0.010, 0.012)),
    5: dict(name='revolver', weapon='w_revolver_', right=(0.030, 0, -0.080), right_euler=(0, 8, 0),
            left=(0.070, 0.027, -0.094), left_euler=(0, -8, 0), trigger=(0.130, -0.010, -0.077)),
    6: dict(name='sniper', weapon='w_sniper', right=(-0.010, 0, -0.042), right_euler=(0, 8, 0),
            left=(0.385, 0, 0.063), left_euler=(-90, 0, 0), trigger=(0.101, -0.012, -0.009)),
    7: dict(name='rpg7', weapon='w_rpg7', right=(-0.027, 0, -0.025), right_euler=(0, 4, 0),
            left=(-0.190, 0, 0.027), left_euler=(0, 0, 0), trigger=(0.012, -0.013, 0.023)),
}


def say(message):
    print(f'[grips {time.perf_counter()-START:.1f}s] {message}', flush=True)


def vec(values):
    return [round(float(value), 6) for value in values]


def engine_point(point):
    return vec(AXIS_TO_ENGINE @ point)


def load_library():
    spec = importlib.util.spec_from_file_location('grip_source_v16', REPO/'tools/v16/build_operator_viewmodel.py')
    library = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(library)
    return library


def mesh_tree(objects):
    vertices, faces = [], []
    for obj in objects:
        start = len(vertices)
        vertices.extend(obj.matrix_world @ vertex.co for vertex in obj.data.vertices)
        faces.extend([start + index for index in polygon.vertices] for polygon in obj.data.polygons)
    return BVHTree.FromPolygons(vertices, faces)


def import_weapons():
    before = set(bpy.data.objects)
    bpy.ops.import_scene.gltf(filepath=str(REPO/'crates/arena/assets/viewmodel.glb'))
    imported = [obj for obj in bpy.data.objects if obj not in before]
    weapons = {}
    for weapon_id, profile in PROFILES.items():
        weapons[weapon_id] = [obj for obj in imported if obj.type == 'MESH' and
            (obj.name == profile['weapon'] or (weapon_id == 5 and obj.name.startswith(profile['weapon'])))]
        if not weapons[weapon_id]:
            raise RuntimeError(f'No weapon geometry for {profile["name"]}')
    for obj in imported:
        obj.hide_render = True
    return weapons


def skin_face_ownership(body, short):
    names = {group.index: short(group.name) for group in body.vertex_groups}
    owners = {}
    for vertex in body.data.vertices:
        if not vertex.groups:
            continue
        name = names[max(vertex.groups, key=lambda group: group.weight).group]
        for side in ('Right', 'Left'):
            if name.startswith(side+'Hand'):
                owners[vertex.index] = side
    faces = {}
    for polygon in body.data.polygons:
        votes = Counter(owners[index] for index in polygon.vertices if index in owners)
        # A majority vote drags the source's huge sleeve triangles into the
        # glove. Keep complete hand-owned faces; the cuff seals that boundary.
        if votes and sum(votes.values()) == len(polygon.vertices):
            faces[polygon.index] = votes.most_common(1)[0][0]
    return faces


class Pose:
    def __init__(self, arm, body, hold, to_rifle, library):
        self.arm, self.body, self.library = arm, body, library
        self.to_weapon = to_rifle.to_4x4() @ Matrix.Translation(-hold)
        self.arm_to_weapon = self.to_weapon @ arm.matrix_world
        self.weapon_to_arm = self.arm_to_weapon.inverted()
        self.bones = {library.short(bone.name): bone for bone in arm.pose.bones}
        self.original = {bone.name: bone.matrix_basis.copy() for bone in arm.pose.bones}
        self.source_matrix = {side: self.arm_to_weapon @ self.bones[side+'ForeArm'].matrix.copy() for side in ('Right','Left')}
        self.faces = skin_face_ownership(body, library.short)

    def point(self, name, tail=False):
        bone = self.bones[name]
        return self.arm_to_weapon @ (bone.tail if tail else bone.head)

    def reset(self):
        for bone in self.arm.pose.bones:
            bone.matrix_basis = self.original[bone.name]
        bpy.context.view_layer.update()

    def place(self, side, profile):
        key = side.lower()
        rotation = Euler(tuple(math.radians(value) for value in profile[key+'_euler']), 'XYZ').to_matrix().to_4x4()
        delta = Matrix.Translation(Vector(profile[key])) @ rotation @ Matrix.Translation(-SOURCE_GRIPS[side])
        # Transform the forearm parent too: vertices near the cut wrist may
        # carry a minority forearm weight, and must follow the glove socket.
        self.bones[side+'ForeArm'].matrix = self.weapon_to_arm @ delta @ self.source_matrix[side]
        bpy.context.view_layer.update()

    def solve_finger(self, side, finger, target):
        chain = [self.bones[side+'Hand'+finger+str(index)] for index in (1, 2, 3)]
        for bone in chain:
            orientation = bone.matrix_basis.to_quaternion().to_euler('XYZ')
            bone.rotation_mode = 'XYZ'
            bone.rotation_euler = orientation
        original = [bone.rotation_euler.copy() for bone in chain]
        if finger == 'Thumb':
            parameters = [(index, axis) for index in range(3) for axis in (0, 1, 2)]
        else:
            parameters = [(0, 0), (1, 0), (2, 0), (0, 1), (0, 2)]
        tip_name = side+'Hand'+finger+'3'

        def loss():
            distance = (self.point(tip_name, True)-target).length_squared
            # Prefer the artist's spread/twist when equally good contact is
            # available. MCP/PIP/DIP flexion is explicitly fitted per weapon.
            regularizer = sum((bone.rotation_euler[axis]-original[index][axis])**2
                for index, bone in enumerate(chain) for axis in (1, 2))
            return distance + regularizer * 0.000002

        best_pose, best_loss = [bone.rotation_euler.copy() for bone in chain], float('inf')
        seeds = [None, (20, 65, 35), (55, 65, 40)] if finger == 'Index' else [None]
        for seed in seeds:
            for index,bone in enumerate(chain):
                bone.rotation_euler = original[index]
                if seed is not None: bone.rotation_euler.x = math.radians(seed[index])
                if finger != 'Thumb':
                    # The artist pose itself can sit a few degrees outside
                    # our limits; clamp the starting point as well as trials.
                    lower=math.radians(-15 if index == 0 else 0)
                    upper=math.radians((105,115,85)[index])
                    bone.rotation_euler.x=max(lower,min(upper,bone.rotation_euler.x))
            bpy.context.view_layer.update()
            for step_degrees in (24, 12, 6, 3, 1):
                step = math.radians(step_degrees)
                for _sweep in range(2):
                    for index, axis in parameters:
                        bone = chain[index]
                        current = bone.rotation_euler[axis]
                        best, score = current, loss()
                        if finger != 'Thumb' and axis == 0:
                            limits = (math.radians(-15 if index == 0 else 0), math.radians((105, 115, 85)[index]))
                        else:
                            spread = math.radians(45 if finger == 'Thumb' else 22)
                            limits = (original[index][axis]-spread, original[index][axis]+spread)
                        for candidate in (max(limits[0], current-step), min(limits[1], current+step)):
                            bone.rotation_euler[axis] = candidate
                            bpy.context.view_layer.update()
                            candidate_score = loss()
                            if candidate_score < score:
                                best, score = candidate, candidate_score
                        bone.rotation_euler[axis] = best
                        bpy.context.view_layer.update()
            if loss() < best_loss:
                best_loss = loss()
                best_pose = [bone.rotation_euler.copy() for bone in chain]
        for bone,angles in zip(chain,best_pose): bone.rotation_euler=angles
        bpy.context.view_layer.update()
        return dict(target=engine_point(target), tip=engine_point(self.point(tip_name, True)),
                    error_mm=round((self.point(tip_name, True)-target).length*1000, 2),
                    curl_degrees=[vec(math.degrees(angle) for angle in bone.rotation_euler) for bone in chain])

    def extract(self, side, name):
        depsgraph = bpy.context.evaluated_depsgraph_get()
        mesh = bpy.data.meshes.new_from_object(self.body.evaluated_get(depsgraph), preserve_all_data_layers=True, depsgraph=depsgraph)
        bm = bmesh.new()
        bm.from_mesh(mesh)
        bm.faces.ensure_lookup_table()
        bmesh.ops.delete(bm, geom=[face for face in bm.faces if self.faces.get(face.index) != side], context='FACES')
        bmesh.ops.delete(bm, geom=[vertex for vertex in bm.verts if not vertex.link_faces], context='VERTS')
        bm.to_mesh(mesh)
        bm.free()
        mesh.transform(self.to_weapon @ self.body.matrix_world)
        mesh.name = name
        obj = bpy.data.objects.new(name, mesh)
        bpy.context.scene.collection.objects.link(obj)
        obj.hide_render = True
        if len(mesh.uv_layers) != 1:
            keep = mesh.uv_layers.active or mesh.uv_layers[0]
            for layer in list(mesh.uv_layers):
                if layer != keep:
                    mesh.uv_layers.remove(layer)
        mesh.uv_layers[0].name = 'UVMap'
        for polygon in mesh.polygons:
            polygon.use_smooth = True
        return obj

    def socket(self, side, node, profile, contacts):
        wrist = self.point(side+'Hand')
        knuckle = self.point(side+'HandMiddle1')
        palm = (wrist+knuckle)*0.5
        bone_matrix = self.arm_to_weapon @ self.bones[side+'Hand'].matrix
        rotation = (AXIS_TO_ENGINE @ bone_matrix.to_3x3()).to_quaternion()
        return dict(node=node, wrist=engine_point(wrist), rotation=vec((rotation.x, rotation.y, rotation.z, rotation.w)),
                    palm=engine_point(palm), wrist_forward=engine_point((knuckle-wrist).normalized()), grip_target=engine_point(Vector(profile[side.lower()])),
                    hand_length=0.20, wrist_width=0.065, forearm_length=0.27, upper_arm_length=0.29,
                    fingers=contacts)


def fit_hand(pose, side, profile, tree):
    contacts = {}
    for finger in FINGERS:
        marker = pose.point(side+'Hand'+finger+'3', True)
        if side == 'Right' and finger == 'Index':
            target = Vector(profile['trigger'])
        else:
            near, normal, _index, distance = tree.find_nearest(marker)
            # Bone tip is beneath the glove surface. Keep the bone outside
            # the prop by a glove-pad radius instead of burying the mesh.
            target = near + normal*0.005
        contacts[finger.lower()] = pose.solve_finger(side, finger, target)
    return contacts


def select_only(objects):
    bpy.ops.object.select_all(action='DESELECT')
    for obj in objects:
        obj.select_set(True)
    bpy.context.view_layer.objects.active = objects[0]


def bake_atlas(objects, source_picture, directory):
    """Bake one compact glove-only atlas, preserving source texture detail.

    Every variant retains identical loop ordering, so the atlas coordinates
    from its reference hand transfer without rebaking or projecting fingers.
    """
    refs = [objects[0]['right'], objects[0]['left']]
    originals = {}
    for side, obj in zip(('right','left'), refs):
        originals[side] = len(obj.data.loops)
    copies = []
    for obj in refs:
        copy = obj.copy(); copy.data = obj.data.copy()
        copy.hide_render = False
        bpy.context.scene.collection.objects.link(copy); copies.append(copy)
    select_only(copies)
    bpy.ops.object.join()
    joined = bpy.context.object
    source_uv = joined.data.uv_layers[0]
    source_uv.name = 'SourceUV'
    source_values = [tuple(loop.uv) for loop in source_uv.data]
    atlas_uv = joined.data.uv_layers.new(name='AtlasUV')
    for loop, values in zip(atlas_uv.data, source_values):
        loop.uv = values
    joined.data.uv_layers.active_index = 1
    joined.data.uv_layers['AtlasUV'].active_render = True
    bpy.ops.object.mode_set(mode='EDIT'); bpy.ops.mesh.select_all(action='SELECT')
    bpy.ops.uv.select_all(action='SELECT'); bpy.ops.uv.pack_islands(margin_method='ADD', margin=0.008, rotate=True)
    bpy.ops.object.mode_set(mode='OBJECT')
    picture = bpy.data.images.load(str(source_picture), check_existing=False)
    picture.colorspace_settings.name = 'sRGB'
    atlas = bpy.data.images.new('glove-atlas-512', ATLAS_SIZE, ATLAS_SIZE, alpha=False)
    atlas.colorspace_settings.name = 'sRGB'
    material = bpy.data.materials.new('glove-bake'); material.use_nodes = True
    nodes, links = material.node_tree.nodes, material.node_tree.links
    bsdf = nodes.get('Principled BSDF')
    src = nodes.new('ShaderNodeTexImage'); src.image = picture
    uv = nodes.new('ShaderNodeUVMap'); uv.uv_map = 'SourceUV'
    links.new(uv.outputs['UV'], src.inputs['Vector']); links.new(src.outputs['Color'], bsdf.inputs['Base Color'])
    target = nodes.new('ShaderNodeTexImage'); target.image = atlas
    for node in nodes: node.select = False
    target.select = True; nodes.active = target
    joined.data.materials.clear(); joined.data.materials.append(material)
    for polygon in joined.data.polygons: polygon.material_index = 0
    scene = bpy.context.scene; scene.render.engine = 'CYCLES'; scene.cycles.samples = 8
    scene.cycles.device = 'CPU'
    scene.render.bake.use_pass_direct = False; scene.render.bake.use_pass_indirect = False; scene.render.bake.use_pass_color = True
    scene.render.bake.margin = 4; scene.render.bake.use_selected_to_active = False
    select_only([joined]); bpy.ops.object.bake(type='DIFFUSE')
    atlas_path = directory/'glove-atlas-512.png'
    atlas.filepath_raw = str(atlas_path); atlas.file_format = 'PNG'; atlas.save(); atlas.pack()
    atlas_values = [tuple(loop.uv) for loop in joined.data.uv_layers['AtlasUV'].data]
    split = originals['right']
    by_side = {'right':atlas_values[:split], 'left':atlas_values[split:]}
    final_material = bpy.data.materials.new('operator-gloves'); final_material.use_nodes = True
    bsdf = final_material.node_tree.nodes.get('Principled BSDF'); bsdf.inputs['Base Color'].default_value = (1,1,1,1)
    bsdf.inputs['Roughness'].default_value = 0.72
    image = final_material.node_tree.nodes.new('ShaderNodeTexImage'); image.image = atlas
    final_material.node_tree.links.new(image.outputs['Color'], bsdf.inputs['Base Color'])
    for pair in objects:
        for side, obj in pair.items():
            if len(obj.data.loops) != len(by_side[side]): raise RuntimeError('Pose changed glove topology')
            for loop, values in zip(obj.data.uv_layers[0].data, by_side[side]): loop.uv = values
            obj.data.materials.clear(); obj.data.materials.append(final_material)
            for polygon in obj.data.polygons: polygon.material_index = 0
    bpy.data.objects.remove(joined, do_unlink=True)
    say(f'Glove-only atlas baked once at {ATLAS_SIZE}²; all 12 hands share the embedded PNG')


def preview(weapon_id, objects, directory):
    scene = bpy.context.scene
    scene.render.engine = 'BLENDER_WORKBENCH'
    scene.display.shading.light = 'STUDIO'; scene.display.shading.color_type = 'TEXTURE'
    scene.display.shading.studio_light = 'paint.sl'
    scene.display.shading.show_specular_highlight = False
    scene.display.shading.background_type = 'WORLD'
    if scene.world is None: scene.world=bpy.data.worlds.new('inspection-world')
    scene.world.color=(0.18,0.18,0.18)
    scene.display.shading.show_shadows = True; scene.display.shading.show_cavity = True
    scene.display.shading.cavity_type = 'BOTH'
    scene.render.resolution_x = 1024; scene.render.resolution_y = 768; scene.render.resolution_percentage = 100
    scene.render.image_settings.file_format = 'PNG'
    for obj in bpy.data.objects: obj.hide_render = obj.type != 'CAMERA'
    for obj in objects: obj.hide_render = False
    camera_data = bpy.data.cameras.new('grip-inspection-camera')
    camera = bpy.data.objects.new('grip-inspection-camera', camera_data)
    scene.collection.objects.link(camera); scene.camera = camera
    # Inspect contact rather than fitting the full barrel into every image.
    target = Vector((0.12, 0, -0.040))
    for name, offset in {'right':(-0.18,-0.90,0.23),'left':(-0.12,0.90,0.20),'eye':(-0.67,0.20,0.19)}.items():
        camera.location = target+Vector(offset)
        camera.rotation_euler = (target-camera.location).to_track_quat('-Z','Y').to_euler()
        camera_data.type = 'PERSP'; camera_data.lens = 52 if name != 'eye' else 40
        scene.render.filepath = str(directory/f'grip-{weapon_id}-{name}.png')
        bpy.ops.render.render(write_still=True)
    bpy.data.objects.remove(camera, do_unlink=True)


def trim_wrist(obj, socket):
    """Clip the source's sparse sleeve triangles at a true wrist plane.

    Majority bone ownership leaves a few long forearm triangle tips attached
    to the glove. A geometric cut removes those without discarding fingers;
    the tiny cloth cuff bridges the runtime sleeve to the cut glove boundary.
    """
    to_blender=AXIS_TO_ENGINE.transposed()
    wrist=to_blender @ Vector(socket['wrist'])
    forward=(to_blender @ Vector(socket['wrist_forward'])).normalized()
    plane=wrist+forward*0.004
    bm=bmesh.new(); bm.from_mesh(obj.data)
    result=bmesh.ops.bisect_plane(bm,geom=list(bm.verts)+list(bm.edges)+list(bm.faces),
        dist=0.000001,plane_co=plane,plane_no=forward,clear_inner=True,clear_outer=False)
    # Skinning can flare a sparse boundary vertex beyond the cuff even after
    # the sleeve is cut away. Taper only the first 30 mm of glove into a clean
    # seam; palm and finger geometry beyond that seam remain unmodified.
    for vertex in bm.verts:
        offset=vertex.co-wrist
        along=offset.dot(forward)
        if along<0.030:
            radial=offset-forward*along
            radius=0.032+0.008*max(0.0,min(1.0,(along-0.004)/0.026))
            if radial.length>radius:
                vertex.co=wrist+forward*along+radial.normalized()*radius
    edges=[edge for edge in result['geom_cut'] if isinstance(edge,bmesh.types.BMEdge) and edge.is_boundary]
    if edges: bmesh.ops.holes_fill(bm,edges=edges,sides=0)
    uv_layer=bm.loops.layers.uv.active
    # A stitched cuff, only 22mm long, rather than another fake forearm.
    sample=min(bm.verts,key=lambda vertex:(vertex.co-wrist).length_squared)
    cuff_uv=sample.link_loops[0][uv_layer].uv.copy()
    cuff=bmesh.ops.create_cone(bm,cap_ends=True,cap_tris=False,segments=16,
        radius1=0.034,radius2=0.032,depth=0.022)
    rot=forward.to_track_quat('Z','Y').to_matrix()
    for vertex in cuff['verts']:
        vertex.co=rot@vertex.co+wrist+forward*0.004
    cuff_vertices=set(cuff['verts'])
    for face in bm.faces:
        if all(vertex in cuff_vertices for vertex in face.verts):
            for loop in face.loops: loop[uv_layer].uv=cuff_uv
            face.smooth=True
    bmesh.ops.recalc_face_normals(bm,faces=list(bm.faces))
    bm.to_mesh(obj.data); bm.free()


def verify(path, metadata):
    for profile in metadata['weapons'].values():
        for hand in profile.values():
            for field in ('wrist','palm','grip_target','wrist_forward','rotation'):
                if not all(math.isfinite(value) for value in hand[field]):
                    raise RuntimeError(f'Non-finite {field} in {hand["node"]}')
            if abs(sum(value*value for value in hand['rotation'])-1.0)>0.001:
                raise RuntimeError(f'Non-unit wrist rotation in {hand["node"]}')
            for finger,contact in hand['fingers'].items():
                # This measures the articulated bone tip against its padded
                # surface/trigger target, not an unsubstantiated promise of
                # zero skin penetration. Inspect the accompanying previews.
                if not math.isfinite(contact['error_mm']) or contact['error_mm']>20:
                    raise RuntimeError(f'Finger target missed: {hand["node"]} {finger} {contact["error_mm"]} mm')
    data = path.read_bytes()
    magic, version, total = struct.unpack_from('<III', data)
    if magic != 0x46546c67 or version != 2 or total != len(data): raise RuntimeError('Invalid GLB header')
    size, kind = struct.unpack_from('<II', data, 12)
    if kind != 0x4e4f534a: raise RuntimeError('GLB JSON chunk missing')
    doc = json.loads(data[20:20+size])
    expected = {hand['node'] for profile in metadata['weapons'].values() for hand in profile.values()}
    actual = {node.get('name') for node in doc['nodes'] if 'mesh' in node}
    if actual != expected: raise RuntimeError(f'Incorrect hand node set: {actual}')
    if doc.get('skins'): raise RuntimeError('Output must contain evaluated static hands, not hidden skinning')
    triangles = 0
    for mesh in doc['meshes']:
        for primitive in mesh['primitives']:
            attributes = primitive['attributes']
            if 'NORMAL' not in attributes or 'TEXCOORD_0' not in attributes or 'TEXCOORD_1' in attributes: raise RuntimeError('Hand requires normals and exactly one UV layer')
            position=doc['accessors'][attributes['POSITION']]
            if not all(math.isfinite(value) and abs(value)<1.0 for value in position['min']+position['max']):
                raise RuntimeError('Hand bounds are non-finite or exceed plausible weapon-local metres')
            material = doc['materials'][primitive['material']]['pbrMetallicRoughness']
            if 'baseColorTexture' not in material or material.get('baseColorFactor',[1,1,1,1]) != [1,1,1,1]: raise RuntimeError('Gloves must have white factor and albedo texture')
            triangles += doc['accessors'][primitive['indices']]['count']//3
    binary_start = 20+size+8
    for image in doc['images']:
        view = doc['bufferViews'][image['bufferView']]
        png = data[binary_start+view.get('byteOffset',0):binary_start+view.get('byteOffset',0)+view['byteLength']]
        if png[:8] != b'\x89PNG\r\n\x1a\n': raise RuntimeError('Glove image is not PNG')
        width,height,depth = struct.unpack_from('>IIB',png,16)
        if width>ATLAS_SIZE or height>ATLAS_SIZE or depth!=8: raise RuntimeError(f'Unsupported PNG {width}x{height}, depth {depth}')
    if len(data)>2_000_000 or triangles>25_000: raise RuntimeError(f'Grip budget exceeded: {len(data)} bytes / {triangles} triangles')
    say(f'Verified {len(actual)} glove nodes, {triangles} triangles, {len(doc["images"])} PNG, {len(data)} bytes')


def main():
    args = argparse.ArgumentParser(description=__doc__)
    args.add_argument('--source-root',type=Path,default=REPO/'assets')
    args.add_argument('--preview-dir',type=Path,default=REPO/'target/grip-previews')
    args.add_argument('--no-preview',action='store_true')
    options = args.parse_args(sys.argv[sys.argv.index('--')+1:] if '--' in sys.argv else [])
    options.preview_dir.mkdir(parents=True,exist_ok=True)
    library = load_library()
    library.SRC = str(options.source_root/'swat/source/swat lp.fbx')
    library.BODY_PICTURE = str(options.source_root/'swat/baked/body-2048.png')
    arm,body,rifle_source = library.import_operator()
    hold,to_rifle,_muzzle = library.rifle_frame(rifle_source)
    # Source forearms are connected bones, so Blender discards their pose
    # translation. Detach the two bake controls without moving their rest
    # heads/tails; the source file itself remains untouched.
    select_only([arm]); bpy.ops.object.mode_set(mode='EDIT')
    for bone in arm.data.edit_bones:
        if library.short(bone.name) in ('LeftForeArm','RightForeArm'):
            bone.use_connect = False
    bpy.ops.object.mode_set(mode='OBJECT'); bpy.context.view_layer.update()
    for obj in rifle_source: bpy.data.objects.remove(obj,do_unlink=True)
    pose = Pose(arm,body,hold,to_rifle,library)
    weapons = import_weapons()
    all_pairs=[]; metadata={'schema':1,'space':'engine +X forward, +Y up, +Z right; static vertices in weapon-local space',
        'source':'SWAT operator original glove rig; per-weapon articulated fingers; 512px glove-only atlas',
        'fallbacks':{'4':1},'weapons':{}}
    for weapon_id,profile in PROFILES.items():
        pose.reset()
        for side in ('Right','Left'): pose.place(side,profile)
        weapon_tree=mesh_tree(weapons[weapon_id])
        pair={}; sockets={}
        for side in ('Right','Left'):
            # Revolver support fingers cup the firing hand rather than
            # reaching forward to the cylinder or wrapping empty air.
            contact_tree=mesh_tree([pair['right']]) if weapon_id==5 and side=='Left' else weapon_tree
            contacts=fit_hand(pose,side,profile,contact_tree)
            name=f'grip_{weapon_id}_{side[0].lower()}'
            obj=pose.extract(side,name); pair[side.lower()]=obj
            sockets[side.lower()]=pose.socket(side,name,profile,contacts)
            say(f'{profile["name"]} {side}: wrist {sockets[side.lower()]["wrist"]}; finger error mm '+str({name:value['error_mm'] for name,value in contacts.items()}))
        all_pairs.append(pair); metadata['weapons'][str(weapon_id)]=sockets
    bake_atlas(all_pairs,options.source_root/'swat/baked/body-2048.png',options.preview_dir)
    for (weapon_id,_profile),pair in zip(PROFILES.items(),all_pairs):
        for side,obj in pair.items(): trim_wrist(obj,metadata['weapons'][str(weapon_id)][side])
    objects=[obj for pair in all_pairs for obj in pair.values()]
    output=REPO/'crates/arena/assets/weapon-grips.glb'
    select_only(objects)
    bpy.ops.export_scene.gltf(filepath=str(output),export_format='GLB',export_yup=True,use_selection=True,
        export_apply=True,export_image_format='AUTO',export_skins=False,export_animations=False)
    verify(output,metadata)
    (REPO/'crates/arena/assets/weapon-grips.json').write_text(json.dumps(metadata,indent=2)+'\n',encoding='utf-8',newline='\n')
    if not options.no_preview:
        for (weapon_id,_profile),pair in zip(PROFILES.items(),all_pairs):
            preview(weapon_id,weapons[weapon_id]+list(pair.values()),options.preview_dir)
    say('Complete; no original weapon geometry or muzzle sockets were changed')


if __name__=='__main__':
    main()
