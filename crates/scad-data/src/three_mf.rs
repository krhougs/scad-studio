use std::{
    collections::HashMap,
    fmt,
    fs::File,
    io::{BufReader, Read, Seek},
    path::Path,
};

use glam::Vec3;
use roxmltree::{Document, Node};
use zip::ZipArchive;

use scene::mesh::{self, MeshData, MeshTriangle};

const MODEL_ENTRY_PATH: &str = "3D/3dmodel.model";

#[derive(Debug)]
pub struct ThreeMfError(String);

#[derive(Clone)]
struct ModelData {
    resources: HashMap<u32, PropertyResource>,
    objects: HashMap<u32, MeshObject>,
    build_items: Vec<BuildItem>,
}

#[derive(Clone)]
struct MeshObject {
    default_property: Option<DefaultProperty>,
    vertices: Vec<[f32; 3]>,
    triangles: Vec<ObjectTriangle>,
}

#[derive(Clone, Copy)]
struct DefaultProperty {
    resource_id: u32,
    index: u32,
}

#[derive(Clone)]
struct ObjectTriangle {
    indices: [usize; 3],
    property: Option<TriangleProperty>,
}

#[derive(Clone, Copy)]
struct TriangleProperty {
    resource_id: u32,
    first: Option<u32>,
    second: Option<u32>,
    third: Option<u32>,
}

#[derive(Clone)]
enum PropertyResource {
    BaseMaterials(Vec<[f32; 4]>),
    ColorGroup(Vec<[f32; 4]>),
}

#[derive(Clone, Copy)]
struct BuildItem {
    object_id: u32,
    transform: Transform3d,
}

#[derive(Clone, Copy)]
struct Transform3d {
    rows: [[f32; 4]; 3],
}

pub fn load_3mf(path: &Path) -> Result<MeshData, ThreeMfError> {
    let file =
        File::open(path).map_err(|error| ThreeMfError(format!("打开 3MF 失败: {error}")))?;
    let mut reader = BufReader::new(file);
    load_3mf_from_reader(&mut reader)
}

pub fn load_3mf_from_reader<R>(reader: &mut R) -> Result<MeshData, ThreeMfError>
where
    R: Read + Seek,
{
    let xml = read_model_xml(reader)?;
    let model = parse_model(&xml)?;
    build_mesh_data(&model)
}

fn read_model_xml<R>(reader: &mut R) -> Result<String, ThreeMfError>
where
    R: Read + Seek,
{
    let mut archive = ZipArchive::new(reader)
        .map_err(|error| ThreeMfError(format!("解析 3MF ZIP 失败: {error}")))?;
    let mut entry = archive
        .by_name(MODEL_ENTRY_PATH)
        .map_err(|_| ThreeMfError(format!("3MF 中缺少必需模型文件 /{MODEL_ENTRY_PATH}")))?;
    let mut xml = String::new();
    entry
        .read_to_string(&mut xml)
        .map_err(|error| ThreeMfError(format!("读取 3MF 模型 XML 失败: {error}")))?;
    Ok(xml)
}

fn parse_model(xml: &str) -> Result<ModelData, ThreeMfError> {
    let doc = Document::parse(xml)
        .map_err(|error| ThreeMfError(format!("解析 3MF XML 失败: {error}")))?;
    let root = doc.root_element();
    let resources_node = child_by_name(root, "resources")
        .ok_or_else(|| ThreeMfError("3MF 中缺少 resources 节点".into()))?;
    Ok(ModelData {
        resources: parse_resources(resources_node)?,
        objects: parse_objects(resources_node)?,
        build_items: parse_build_items(root)?,
    })
}

fn parse_resources(
    resources_node: Node<'_, '_>,
) -> Result<HashMap<u32, PropertyResource>, ThreeMfError> {
    let mut resources = HashMap::new();
    for child in resources_node.children().filter(Node::is_element) {
        match child.tag_name().name() {
            "basematerials" => {
                let id = required_u32_attr(child, "id")?;
                let colors = parse_palette(child, "base", "displaycolor")?;
                resources.insert(id, PropertyResource::BaseMaterials(colors));
            }
            "colorgroup" => {
                let id = required_u32_attr(child, "id")?;
                let colors = parse_palette(child, "color", "color")?;
                resources.insert(id, PropertyResource::ColorGroup(colors));
            }
            "object" => {}
            other => {
                return Err(ThreeMfError(format!(
                    "3MF 引用了当前不支持的资源类型 {other}"
                )));
            }
        }
    }
    Ok(resources)
}

fn parse_objects(
    resources_node: Node<'_, '_>,
) -> Result<HashMap<u32, MeshObject>, ThreeMfError> {
    let mut objects = HashMap::new();
    for object_node in children_by_name(resources_node, "object") {
        let object_id = required_u32_attr(object_node, "id")?;
        objects.insert(object_id, parse_object(object_node)?);
    }
    Ok(objects)
}

fn parse_object(object_node: Node<'_, '_>) -> Result<MeshObject, ThreeMfError> {
    let mesh_node = child_by_name(object_node, "mesh").ok_or_else(|| {
        let object_id = object_node.attribute("id").unwrap_or("unknown");
        ThreeMfError(format!("3MF object {object_id} 不是当前支持的 mesh 对象"))
    })?;
    Ok(MeshObject {
        default_property: parse_default_property(object_node)?,
        vertices: parse_vertices(mesh_node)?,
        triangles: parse_triangles(mesh_node)?,
    })
}

fn parse_default_property(node: Node<'_, '_>) -> Result<Option<DefaultProperty>, ThreeMfError> {
    let Some(resource_id) = optional_u32_attr(node, "pid")? else {
        return Ok(None);
    };
    let index = required_u32_attr(node, "pindex")?;
    Ok(Some(DefaultProperty { resource_id, index }))
}

fn parse_vertices(mesh_node: Node<'_, '_>) -> Result<Vec<[f32; 3]>, ThreeMfError> {
    let vertices_node = child_by_name(mesh_node, "vertices")
        .ok_or_else(|| ThreeMfError("3MF mesh 中缺少 vertices 节点".into()))?;
    children_by_name(vertices_node, "vertex")
        .into_iter()
        .map(parse_vertex)
        .collect()
}

fn parse_vertex(node: Node<'_, '_>) -> Result<[f32; 3], ThreeMfError> {
    Ok([
        required_f32_attr(node, "x")?,
        required_f32_attr(node, "y")?,
        required_f32_attr(node, "z")?,
    ])
}

fn parse_triangles(mesh_node: Node<'_, '_>) -> Result<Vec<ObjectTriangle>, ThreeMfError> {
    let triangles_node = child_by_name(mesh_node, "triangles")
        .ok_or_else(|| ThreeMfError("3MF mesh 中缺少 triangles 节点".into()))?;
    children_by_name(triangles_node, "triangle")
        .into_iter()
        .map(parse_triangle)
        .collect()
}

fn parse_triangle(node: Node<'_, '_>) -> Result<ObjectTriangle, ThreeMfError> {
    Ok(ObjectTriangle {
        indices: [
            required_usize_attr(node, "v1")?,
            required_usize_attr(node, "v2")?,
            required_usize_attr(node, "v3")?,
        ],
        property: parse_triangle_property(node)?,
    })
}

fn parse_triangle_property(
    node: Node<'_, '_>,
) -> Result<Option<TriangleProperty>, ThreeMfError> {
    let Some(resource_id) = optional_u32_attr(node, "pid")? else {
        return Ok(None);
    };
    Ok(Some(TriangleProperty {
        resource_id,
        first: optional_u32_attr(node, "p1")?,
        second: optional_u32_attr(node, "p2")?,
        third: optional_u32_attr(node, "p3")?,
    }))
}

fn parse_build_items(root: Node<'_, '_>) -> Result<Vec<BuildItem>, ThreeMfError> {
    let build_node = child_by_name(root, "build")
        .ok_or_else(|| ThreeMfError("3MF 中缺少 build 节点".into()))?;
    let items = children_by_name(build_node, "item")
        .into_iter()
        .map(parse_build_item)
        .collect::<Result<Vec<_>, _>>()?;
    if items.is_empty() {
        return Err(ThreeMfError("3MF build 中没有可渲染的对象".into()));
    }
    Ok(items)
}

fn parse_build_item(node: Node<'_, '_>) -> Result<BuildItem, ThreeMfError> {
    Ok(BuildItem {
        object_id: required_u32_attr(node, "objectid")?,
        transform: Transform3d::from_attr(node.attribute("transform"))?,
    })
}

fn parse_palette(
    resource_node: Node<'_, '_>,
    entry_name: &str,
    color_attr: &str,
) -> Result<Vec<[f32; 4]>, ThreeMfError> {
    children_by_name(resource_node, entry_name)
        .into_iter()
        .map(|node| parse_color(required_attr(node, color_attr)?))
        .collect()
}

fn build_mesh_data(model: &ModelData) -> Result<MeshData, ThreeMfError> {
    let mut triangles = Vec::new();
    for item in &model.build_items {
        let object = model.objects.get(&item.object_id).ok_or_else(|| {
            ThreeMfError(format!(
                "3MF build 引用了不存在的 object {}",
                item.object_id
            ))
        })?;
        let object_triangles =
            object_to_mesh_triangles(object, item.transform, &model.resources)?;
        triangles.extend(object_triangles);
    }
    MeshData::from_triangles(&triangles)
        .map_err(|error| ThreeMfError(format!("构建内部网格失败: {error}")))
}

fn object_to_mesh_triangles(
    object: &MeshObject,
    transform: Transform3d,
    resources: &HashMap<u32, PropertyResource>,
) -> Result<Vec<MeshTriangle>, ThreeMfError> {
    object
        .triangles
        .iter()
        .map(|triangle| triangle_to_mesh(object, triangle, transform, resources))
        .collect()
}

fn triangle_to_mesh(
    object: &MeshObject,
    triangle: &ObjectTriangle,
    transform: Transform3d,
    resources: &HashMap<u32, PropertyResource>,
) -> Result<MeshTriangle, ThreeMfError> {
    let mut positions = [
        resolve_vertex_position(&object.vertices, triangle.indices[0], transform)?,
        resolve_vertex_position(&object.vertices, triangle.indices[1], transform)?,
        resolve_vertex_position(&object.vertices, triangle.indices[2], transform)?,
    ];
    let mut colors =
        resolve_triangle_colors(object.default_property, triangle.property, resources)?;
    if transform.is_mirrored() {
        positions.swap(1, 2);
        colors.swap(1, 2);
    }
    let positions = positions.map(mesh::openscad_to_viewer);
    Ok(MeshTriangle {
        positions,
        normal: triangle_normal(positions),
        colors,
    })
}

fn resolve_vertex_position(
    vertices: &[[f32; 3]],
    index: usize,
    transform: Transform3d,
) -> Result<[f32; 3], ThreeMfError> {
    vertices
        .get(index)
        .copied()
        .map(|point| transform.apply(point))
        .ok_or_else(|| {
            ThreeMfError(format!("3MF triangle 引用了不存在的顶点索引 {index}"))
        })
}

fn resolve_triangle_colors(
    default_property: Option<DefaultProperty>,
    triangle_property: Option<TriangleProperty>,
    resources: &HashMap<u32, PropertyResource>,
) -> Result<[Option<[f32; 4]>; 3], ThreeMfError> {
    match triangle_property {
        Some(property) => match resources.get(&property.resource_id) {
            Some(PropertyResource::BaseMaterials(_)) => {
                resolve_basematerial_triangle(resources, default_property, property)
            }
            Some(PropertyResource::ColorGroup(_)) => {
                resolve_colorgroup_triangle(resources, default_property, property)
            }
            None => Err(ThreeMfError(format!(
                "3MF 引用了未知资源组 id={}",
                property.resource_id
            ))),
        },
        None => resolve_default_colors(resources, default_property),
    }
}

fn resolve_basematerial_triangle(
    resources: &HashMap<u32, PropertyResource>,
    default_property: Option<DefaultProperty>,
    property: TriangleProperty,
) -> Result<[Option<[f32; 4]>; 3], ThreeMfError> {
    let Some(index) = property
        .first
        .or_else(|| inherited_index(default_property, property.resource_id))
    else {
        return Err(ThreeMfError(
            "3MF basematerials 三角面缺少 p1，且没有可继承的对象默认属性".into(),
        ));
    };
    if property.second.is_some_and(|value| value != index)
        || property.third.is_some_and(|value| value != index)
    {
        return Err(ThreeMfError(
            "3MF basematerials 不支持在同一三角面内使用不同的 p1/p2/p3".into(),
        ));
    }
    Ok([Some(resolve_color(resources, property.resource_id, index)?); 3])
}

fn resolve_colorgroup_triangle(
    resources: &HashMap<u32, PropertyResource>,
    default_property: Option<DefaultProperty>,
    property: TriangleProperty,
) -> Result<[Option<[f32; 4]>; 3], ThreeMfError> {
    let Some(first) = property
        .first
        .or_else(|| inherited_index(default_property, property.resource_id))
    else {
        return Err(ThreeMfError(
            "3MF colorgroup 三角面缺少 p1，且没有可继承的对象默认属性".into(),
        ));
    };
    if property.first.is_none() && (property.second.is_some() || property.third.is_some()) {
        return Err(ThreeMfError(
            "3MF colorgroup 在缺少 p1 时不能单独提供 p2 或 p3".into(),
        ));
    }
    let second = property.second.unwrap_or(first);
    let third = property.third.unwrap_or(first);
    Ok([
        Some(resolve_color(resources, property.resource_id, first)?),
        Some(resolve_color(resources, property.resource_id, second)?),
        Some(resolve_color(resources, property.resource_id, third)?),
    ])
}

fn resolve_default_colors(
    resources: &HashMap<u32, PropertyResource>,
    default_property: Option<DefaultProperty>,
) -> Result<[Option<[f32; 4]>; 3], ThreeMfError> {
    match default_property {
        Some(default_property) => {
            let color = resolve_color(
                resources,
                default_property.resource_id,
                default_property.index,
            )?;
            Ok([Some(color); 3])
        }
        None => Ok([None; 3]),
    }
}

fn resolve_color(
    resources: &HashMap<u32, PropertyResource>,
    resource_id: u32,
    index: u32,
) -> Result<[f32; 4], ThreeMfError> {
    match resources.get(&resource_id) {
        Some(PropertyResource::BaseMaterials(colors))
        | Some(PropertyResource::ColorGroup(colors)) => colors
            .get(index as usize)
            .copied()
            .ok_or_else(|| ThreeMfError(format!("3MF 资源 {resource_id} 缺少颜色索引 {index}"))),
        None => Err(ThreeMfError(format!(
            "3MF 引用了未知资源组 id={resource_id}"
        ))),
    }
}

fn inherited_index(default_property: Option<DefaultProperty>, resource_id: u32) -> Option<u32> {
    default_property.and_then(|default| {
        if default.resource_id == resource_id {
            Some(default.index)
        } else {
            None
        }
    })
}

fn triangle_normal(positions: [[f32; 3]; 3]) -> [f32; 3] {
    let a = Vec3::from_array(positions[0]);
    let b = Vec3::from_array(positions[1]);
    let c = Vec3::from_array(positions[2]);
    let normal = (b - a).cross(c - a).normalize_or_zero();
    if normal == Vec3::ZERO {
        [0.0, 1.0, 0.0]
    } else {
        normal.to_array()
    }
}

fn parse_color(value: &str) -> Result<[f32; 4], ThreeMfError> {
    let hex = value.strip_prefix('#').unwrap_or(value);
    let bytes = match hex.len() {
        6 => [
            parse_hex_byte(&hex[0..2])?,
            parse_hex_byte(&hex[2..4])?,
            parse_hex_byte(&hex[4..6])?,
            255,
        ],
        8 => [
            parse_hex_byte(&hex[0..2])?,
            parse_hex_byte(&hex[2..4])?,
            parse_hex_byte(&hex[4..6])?,
            parse_hex_byte(&hex[6..8])?,
        ],
        _ => return Err(ThreeMfError(format!("3MF 颜色值格式无效: {value}"))),
    };
    Ok(bytes.map(|byte| byte as f32 / 255.0))
}

fn parse_hex_byte(value: &str) -> Result<u8, ThreeMfError> {
    u8::from_str_radix(value, 16)
        .map_err(|error| ThreeMfError(format!("解析 3MF 颜色值失败: {error}")))
}

fn child_by_name<'a, 'input>(node: Node<'a, 'input>, name: &str) -> Option<Node<'a, 'input>> {
    node.children()
        .find(|child| child.is_element() && child.tag_name().name() == name)
}

fn children_by_name<'a, 'input>(node: Node<'a, 'input>, name: &str) -> Vec<Node<'a, 'input>> {
    node.children()
        .filter(|child| child.is_element() && child.tag_name().name() == name)
        .collect()
}

fn required_attr<'a>(node: Node<'a, 'a>, name: &str) -> Result<&'a str, ThreeMfError> {
    node.attribute(name).ok_or_else(|| {
        ThreeMfError(format!(
            "3MF 节点 {} 缺少属性 {name}",
            node.tag_name().name()
        ))
    })
}

fn required_f32_attr(node: Node<'_, '_>, name: &str) -> Result<f32, ThreeMfError> {
    parse_attr(node, name)?.ok_or_else(|| ThreeMfError(format!("3MF 属性 {name} 缺少数值")))
}

fn required_u32_attr(node: Node<'_, '_>, name: &str) -> Result<u32, ThreeMfError> {
    parse_attr(node, name)?.ok_or_else(|| ThreeMfError(format!("3MF 属性 {name} 缺少数值")))
}

fn required_usize_attr(node: Node<'_, '_>, name: &str) -> Result<usize, ThreeMfError> {
    parse_attr(node, name)?.ok_or_else(|| ThreeMfError(format!("3MF 属性 {name} 缺少数值")))
}

fn optional_u32_attr(node: Node<'_, '_>, name: &str) -> Result<Option<u32>, ThreeMfError> {
    parse_attr(node, name)
}

fn parse_attr<T>(node: Node<'_, '_>, name: &str) -> Result<Option<T>, ThreeMfError>
where
    T: std::str::FromStr,
    T::Err: fmt::Display,
{
    node.attribute(name)
        .map(|value| {
            value.parse::<T>().map_err(|error| {
                ThreeMfError(format!("解析 3MF 属性 {}={} 失败: {error}", name, value))
            })
        })
        .transpose()
}

impl Transform3d {
    fn identity() -> Self {
        Self {
            rows: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
            ],
        }
    }

    fn from_attr(value: Option<&str>) -> Result<Self, ThreeMfError> {
        let Some(value) = value else {
            return Ok(Self::identity());
        };
        let numbers = value
            .split_whitespace()
            .map(|part| {
                part.parse::<f32>()
                    .map_err(|error| ThreeMfError(format!("解析 3MF transform 失败: {error}")))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if numbers.len() != 12 {
            return Err(ThreeMfError(format!(
                "3MF transform 需要 12 个浮点数，实际得到 {} 个",
                numbers.len()
            )));
        }
        Ok(Self {
            rows: [
                [numbers[0], numbers[1], numbers[2], numbers[3]],
                [numbers[4], numbers[5], numbers[6], numbers[7]],
                [numbers[8], numbers[9], numbers[10], numbers[11]],
            ],
        })
    }

    fn apply(self, point: [f32; 3]) -> [f32; 3] {
        [
            self.rows[0][0] * point[0]
                + self.rows[0][1] * point[1]
                + self.rows[0][2] * point[2]
                + self.rows[0][3],
            self.rows[1][0] * point[0]
                + self.rows[1][1] * point[1]
                + self.rows[1][2] * point[2]
                + self.rows[1][3],
            self.rows[2][0] * point[0]
                + self.rows[2][1] * point[1]
                + self.rows[2][2] * point[2]
                + self.rows[2][3],
        ]
    }

    fn is_mirrored(self) -> bool {
        let determinant = self.rows[0][0]
            * (self.rows[1][1] * self.rows[2][2] - self.rows[1][2] * self.rows[2][1])
            - self.rows[0][1]
                * (self.rows[1][0] * self.rows[2][2] - self.rows[1][2] * self.rows[2][0])
            + self.rows[0][2]
                * (self.rows[1][0] * self.rows[2][1] - self.rows[1][1] * self.rows[2][0]);
        determinant < 0.0
    }
}

impl std::error::Error for ThreeMfError {}

impl fmt::Display for ThreeMfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
