use std::io::{Cursor, Write};

use glam::Vec3;
use scad_scene::three_mf;
use zip::{ZipWriter, write::SimpleFileOptions};

#[test]
fn load_3mf_from_reader_applies_object_level_basematerial_colors() {
    let xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<model unit="millimeter" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02">
  <resources>
    <basematerials id="1">
      <base name="Red" displaycolor="#FF0000"/>
      <base name="Green" displaycolor="#00FF00"/>
    </basematerials>
    <object id="10" type="model" pid="1" pindex="0">
      <mesh>
        <vertices>
          <vertex x="0" y="0" z="0"/>
          <vertex x="1" y="0" z="0"/>
          <vertex x="0" y="1" z="0"/>
        </vertices>
        <triangles>
          <triangle v1="0" v2="1" v3="2"/>
        </triangles>
      </mesh>
    </object>
    <object id="20" type="model" pid="1" pindex="1">
      <mesh>
        <vertices>
          <vertex x="2" y="0" z="0"/>
          <vertex x="3" y="0" z="0"/>
          <vertex x="2" y="1" z="0"/>
        </vertices>
        <triangles>
          <triangle v1="0" v2="1" v3="2"/>
        </triangles>
      </mesh>
    </object>
  </resources>
  <build>
    <item objectid="10"/>
    <item objectid="20"/>
  </build>
</model>"##;
    let mut archive = fixture_archive(xml);

    let mesh = three_mf::load_3mf_from_reader(&mut archive).expect("3mf should parse");

    assert_eq!(mesh.vertices.len(), 6);
    assert!(
        mesh.vertices[..3]
            .iter()
            .all(|vertex| vertex.color == [1.0, 0.0, 0.0, 1.0])
    );
    assert!(
        mesh.vertices[3..]
            .iter()
            .all(|vertex| vertex.color == [0.0, 1.0, 0.0, 1.0])
    );
}

#[test]
fn load_3mf_from_reader_supports_colorgroup_vertex_colors() {
    let xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<model unit="millimeter" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02">
  <resources>
    <colorgroup id="2">
      <color color="#FF0000"/>
      <color color="#00FF00"/>
      <color color="#0000FF"/>
    </colorgroup>
    <object id="30" type="model">
      <mesh>
        <vertices>
          <vertex x="2" y="3" z="5"/>
          <vertex x="7" y="11" z="13"/>
          <vertex x="17" y="19" z="29"/>
        </vertices>
        <triangles>
          <triangle v1="0" v2="1" v3="2" pid="2" p1="0" p2="1" p3="2"/>
        </triangles>
      </mesh>
    </object>
  </resources>
  <build>
    <item objectid="30"/>
  </build>
</model>"##;
    let mut archive = fixture_archive(xml);

    let mesh = three_mf::load_3mf_from_reader(&mut archive).expect("3mf should parse");

    assert_eq!(mesh.vertices[0].color, [1.0, 0.0, 0.0, 1.0]);
    assert_eq!(mesh.vertices[1].color, [0.0, 1.0, 0.0, 1.0]);
    assert_eq!(mesh.vertices[2].color, [0.0, 0.0, 1.0, 1.0]);
    let positions = [[2.0, 3.0, 5.0], [7.0, 11.0, 13.0], [17.0, 19.0, 29.0]];
    assert_eq!(mesh.vertices[0].position, [2.0, 3.0, 5.0]);
    assert_eq!(mesh.vertices[1].position, [7.0, 11.0, 13.0]);
    assert_eq!(mesh.vertices[2].position, [17.0, 19.0, 29.0]);
    assert!(approx_eq_normal(
        mesh.vertices[0].normal,
        triangle_normal(positions)
    ));
}

#[test]
fn load_3mf_from_reader_supports_triangle_pid_without_p1_when_object_has_default_property() {
    let xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<model unit="millimeter" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02">
  <resources>
    <basematerials id="3">
      <base name="Red" displaycolor="#FF0000"/>
      <base name="Green" displaycolor="#00FF00"/>
    </basematerials>
    <object id="31" type="model" pid="3" pindex="1">
      <mesh>
        <vertices>
          <vertex x="0" y="0" z="0"/>
          <vertex x="1" y="0" z="0"/>
          <vertex x="0" y="1" z="0"/>
        </vertices>
        <triangles>
          <triangle v1="0" v2="1" v3="2" pid="3"/>
        </triangles>
      </mesh>
    </object>
  </resources>
  <build>
    <item objectid="31"/>
  </build>
</model>"##;
    let mut archive = fixture_archive(xml);

    let mesh = three_mf::load_3mf_from_reader(&mut archive).expect("3mf should parse");

    assert!(
        mesh.vertices
            .iter()
            .all(|vertex| vertex.color == [0.0, 1.0, 0.0, 1.0])
    );
}

#[test]
fn load_3mf_from_reader_keeps_triangle_level_basematerial_colors() {
    let xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<model unit="millimeter" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02">
  <resources>
    <basematerials id="3">
      <base name="Red" displaycolor="#FF0000"/>
      <base name="Green" displaycolor="#00FF00"/>
    </basematerials>
    <object id="40" type="model">
      <mesh>
        <vertices>
          <vertex x="0" y="0" z="0"/>
          <vertex x="1" y="0" z="0"/>
          <vertex x="0" y="1" z="0"/>
          <vertex x="1" y="1" z="0"/>
        </vertices>
        <triangles>
          <triangle v1="0" v2="1" v3="2" pid="3" p1="0"/>
          <triangle v1="1" v2="3" v3="2" pid="3" p1="1"/>
        </triangles>
      </mesh>
    </object>
  </resources>
  <build>
    <item objectid="40"/>
  </build>
</model>"##;
    let mut archive = fixture_archive(xml);

    let mesh = three_mf::load_3mf_from_reader(&mut archive).expect("3mf should parse");

    assert!(
        mesh.vertices[..3]
            .iter()
            .all(|vertex| vertex.color == [1.0, 0.0, 0.0, 1.0])
    );
    assert!(
        mesh.vertices[3..]
            .iter()
            .all(|vertex| vertex.color == [0.0, 1.0, 0.0, 1.0])
    );
}

#[test]
fn load_3mf_from_reader_rejects_gradient_basematerials() {
    let xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<model unit="millimeter" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02">
  <resources>
    <basematerials id="4">
      <base name="Red" displaycolor="#FF0000"/>
      <base name="Green" displaycolor="#00FF00"/>
    </basematerials>
    <object id="41" type="model">
      <mesh>
        <vertices>
          <vertex x="0" y="0" z="0"/>
          <vertex x="1" y="0" z="0"/>
          <vertex x="0" y="1" z="0"/>
        </vertices>
        <triangles>
          <triangle v1="0" v2="1" v3="2" pid="4" p1="0" p2="1" p3="1"/>
        </triangles>
      </mesh>
    </object>
  </resources>
  <build>
    <item objectid="41"/>
  </build>
</model>"##;
    let mut archive = fixture_archive(xml);

    let error = three_mf::load_3mf_from_reader(&mut archive)
        .expect_err("gradient basematerials should fail");

    assert!(error.to_string().contains("basematerials"));
}

#[test]
fn load_3mf_from_reader_rejects_unsupported_material_groups() {
    let xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<model unit="millimeter" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02">
  <resources>
    <texture2dgroup id="7" texid="8"/>
    <object id="50" type="model">
      <mesh>
        <vertices>
          <vertex x="0" y="0" z="0"/>
          <vertex x="1" y="0" z="0"/>
          <vertex x="0" y="1" z="0"/>
        </vertices>
        <triangles>
          <triangle v1="0" v2="1" v3="2" pid="7" p1="0"/>
        </triangles>
      </mesh>
    </object>
  </resources>
  <build>
    <item objectid="50"/>
  </build>
</model>"##;
    let mut archive = fixture_archive(xml);

    let error =
        three_mf::load_3mf_from_reader(&mut archive).expect_err("unsupported resource should fail");

    assert!(error.to_string().contains("texture2dgroup"));
    assert!(error.to_string().contains("不支持"));
}

#[test]
fn load_3mf_from_reader_rejects_unreferenced_unsupported_resources() {
    let xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<model unit="millimeter" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02">
  <resources>
    <texture2dgroup id="7" texid="8"/>
    <object id="60" type="model">
      <mesh>
        <vertices>
          <vertex x="0" y="0" z="0"/>
          <vertex x="1" y="0" z="0"/>
          <vertex x="0" y="1" z="0"/>
        </vertices>
        <triangles>
          <triangle v1="0" v2="1" v3="2"/>
        </triangles>
      </mesh>
    </object>
  </resources>
  <build>
    <item objectid="60"/>
  </build>
</model>"##;
    let mut archive = fixture_archive(xml);

    let error = three_mf::load_3mf_from_reader(&mut archive)
        .expect_err("unsupported resource should fail even if not referenced");

    assert!(error.to_string().contains("texture2dgroup"));
}

#[test]
fn load_3mf_from_reader_preserves_front_face_for_mirrored_build_items() {
    let xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<model unit="millimeter" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02">
  <resources>
    <object id="70" type="model">
      <mesh>
        <vertices>
          <vertex x="0" y="0" z="0"/>
          <vertex x="1" y="0" z="0"/>
          <vertex x="0" y="1" z="0"/>
        </vertices>
        <triangles>
          <triangle v1="0" v2="1" v3="2"/>
        </triangles>
      </mesh>
    </object>
  </resources>
  <build>
    <item objectid="70" transform="-1 0 0 0 0 1 0 0 0 0 1 0"/>
  </build>
</model>"##;
    let mut archive = fixture_archive(xml);

    let mesh = three_mf::load_3mf_from_reader(&mut archive).expect("3mf should parse");

    assert_eq!(mesh.vertices[0].normal, [0.0, 0.0, 1.0]);
}

#[test]
fn load_3mf_from_reader_defaults_degenerate_normals_to_project_up() {
    let xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<model unit="millimeter" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02">
  <resources>
    <object id="75" type="model">
      <mesh>
        <vertices>
          <vertex x="2" y="3" z="5"/>
          <vertex x="2" y="3" z="5"/>
          <vertex x="2" y="3" z="5"/>
        </vertices>
        <triangles>
          <triangle v1="0" v2="1" v3="2"/>
        </triangles>
      </mesh>
    </object>
  </resources>
  <build>
    <item objectid="75"/>
  </build>
</model>"##;
    let mut archive = fixture_archive(xml);

    let mesh = three_mf::load_3mf_from_reader(&mut archive).expect("3mf should parse");

    assert!(
        mesh.vertices
            .iter()
            .all(|vertex| vertex.normal == [0.0, 0.0, 1.0])
    );
}

#[test]
fn load_3mf_from_reader_preserves_zero_alpha_model_colors() {
    let xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<model unit="millimeter" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02">
  <resources>
    <colorgroup id="9">
      <color color="#FF000000"/>
      <color color="#00FF00FF"/>
      <color color="#0000FFFF"/>
    </colorgroup>
    <object id="80" type="model">
      <mesh>
        <vertices>
          <vertex x="0" y="0" z="0"/>
          <vertex x="1" y="0" z="0"/>
          <vertex x="0" y="1" z="0"/>
        </vertices>
        <triangles>
          <triangle v1="0" v2="1" v3="2" pid="9" p1="0" p2="1" p3="2"/>
        </triangles>
      </mesh>
    </object>
  </resources>
  <build>
    <item objectid="80"/>
  </build>
</model>"##;
    let mut archive = fixture_archive(xml);

    let mesh = three_mf::load_3mf_from_reader(&mut archive).expect("3mf should parse");

    assert_eq!(mesh.vertices[0].color, [1.0, 0.0, 0.0, 0.0]);
}

fn fixture_archive(model_xml: &str) -> Cursor<Vec<u8>> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .start_file("3D/3dmodel.model", SimpleFileOptions::default())
        .expect("fixture should open archive entry");
    writer
        .write_all(model_xml.as_bytes())
        .expect("fixture should write xml");
    writer.finish().expect("fixture should finish archive")
}

fn triangle_normal(positions: [[f32; 3]; 3]) -> [f32; 3] {
    let a = Vec3::from_array(positions[0]);
    let b = Vec3::from_array(positions[1]);
    let c = Vec3::from_array(positions[2]);
    (b - a).cross(c - a).normalize().to_array()
}

fn approx_eq_normal(left: [f32; 3], right: [f32; 3]) -> bool {
    Vec3::from_array(left).distance(Vec3::from_array(right)) < 0.0001
}
