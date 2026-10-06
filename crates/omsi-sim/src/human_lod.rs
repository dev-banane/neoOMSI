use crate::human::HumanMesh;
use glam::Vec3;
use omsi_geometry::MeshData;
use std::collections::HashMap;

const SMALL_PART: f32 = 0.04;
const MARGIN: f32 = 0.005;

type Tri = ([u32; 3], u32);

struct Part {
    tris: Vec<usize>,
    lo: Vec3,
    hi: Vec3,
}

fn triangles(d: &MeshData) -> Vec<Tri> {
    d.ranges
        .iter()
        .flat_map(|&(first, count, slot)| {
            d.indices[first as usize..(first + count) as usize]
                .chunks_exact(3)
                .map(move |t| ([t[0], t[1], t[2]], slot))
        })
        .collect()
}

fn small_parts(d: &MeshData, tris: &[Tri]) -> Vec<Part> {
    let n = d.positions.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(p: &mut [usize], mut a: usize) -> usize {
        while p[a] != a {
            p[a] = p[p[a]];
            a = p[a];
        }
        a
    }
    fn union(p: &mut [usize], a: usize, b: usize) {
        let (a, b) = (find(p, a), find(p, b));
        p[b] = a;
    }
    // the corners of a vertex split for its uv or normal lie on the same point
    let mut first: HashMap<[u32; 3], usize> = HashMap::new();
    for (i, p) in d.positions.iter().enumerate() {
        let k = *first.entry(p.to_array().map(f32::to_bits)).or_insert(i);
        union(&mut parent, k, i);
    }
    for (t, _) in tris {
        if t.iter().any(|&i| i as usize >= n) {
            continue;
        }
        union(&mut parent, t[0] as usize, t[1] as usize);
        union(&mut parent, t[0] as usize, t[2] as usize);
    }
    let mut parts: HashMap<usize, Part> = HashMap::new();
    for (k, (t, _)) in tris.iter().enumerate() {
        if t.iter().any(|&i| i as usize >= n) {
            continue;
        }
        let part = parts
            .entry(find(&mut parent, t[0] as usize))
            .or_insert(Part {
                tris: Vec::new(),
                lo: Vec3::splat(f32::MAX),
                hi: Vec3::splat(f32::MIN),
            });
        part.tris.push(k);
        for &i in t {
            part.lo = part.lo.min(d.positions[i as usize]);
            part.hi = part.hi.max(d.positions[i as usize]);
        }
    }
    let mut small: Vec<Part> = parts
        .into_values()
        .filter(|p| (p.hi - p.lo).max_element() <= SMALL_PART)
        .collect();
    small.sort_by_key(|p| p.tris[0]);
    small
}

pub(crate) fn keep_small_parts(low: &mut HumanMesh, top: &HumanMesh) {
    let top_tris = triangles(&top.data);
    let parts = small_parts(&top.data, &top_tris);
    if parts.is_empty() {
        return;
    }
    let low_tris = triangles(&low.data);
    let covered = |p: Vec3| {
        parts
            .iter()
            .any(|q| p.cmpge(q.lo - MARGIN).all() && p.cmple(q.hi + MARGIN).all())
    };
    let mut dropped = vec![false; low_tris.len()];
    for p in small_parts(&low.data, &low_tris) {
        if covered((p.lo + p.hi) * 0.5) {
            for k in p.tris {
                dropped[k] = true;
            }
        }
    }
    if !dropped.contains(&true) {
        return;
    }
    let mut tris: Vec<Tri> = low_tris
        .into_iter()
        .zip(&dropped)
        .filter(|(_, d)| !**d)
        .map(|(t, _)| t)
        .collect();

    let mut slots: HashMap<u32, u32> = HashMap::new();
    let mut vertex: HashMap<u32, u32> = HashMap::new();
    for part in &parts {
        for &k in &part.tris {
            let (t, slot) = top_tris[k];
            let slot = *slots.entry(slot).or_insert_with(|| {
                let (mat, alpha) = (
                    &top.materials[slot as usize],
                    top.alpha.get(slot as usize).copied().unwrap_or(0),
                );
                let same = low.materials.iter().zip(&low.alpha).position(|(m, a)| {
                    m.texture.trim().eq_ignore_ascii_case(mat.texture.trim()) && *a == alpha
                });
                same.unwrap_or_else(|| {
                    low.materials.push(mat.clone());
                    low.alpha.push(alpha);
                    low.materials.len() - 1
                }) as u32
            });
            let t = t.map(|i| {
                *vertex.entry(i).or_insert_with(|| {
                    let d = &mut low.data;
                    let j = i as usize;
                    d.positions.push(top.data.positions[j]);
                    d.normals.push(top.data.normals[j]);
                    d.uvs.push(top.data.uvs[j]);
                    low.skin.push(top.skin[j]);
                    d.positions.len() as u32 - 1
                })
            });
            tris.push((t, slot));
        }
    }
    for (id, weights) in &top.bones {
        let moved: Vec<(u32, f32)> = weights
            .iter()
            .filter_map(|(v, w)| vertex.get(v).map(|n| (*n, *w)))
            .collect();
        if moved.is_empty() {
            continue;
        }
        match low.bones.iter_mut().find(|(b, _)| b == id) {
            Some((_, list)) => list.extend(moved),
            None => low.bones.push((*id, moved)),
        }
    }

    let d = &mut low.data;
    d.indices.clear();
    d.ranges.clear();
    for slot in 0..low.materials.len() as u32 {
        let first = d.indices.len() as u32;
        d.indices.extend(
            tris.iter()
                .filter(|(_, s)| *s == slot)
                .flat_map(|(t, _)| *t),
        );
        let count = d.indices.len() as u32 - first;
        if count > 0 {
            d.ranges.push((first, count, slot));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::human::Influence;

    fn material(texture: &str) -> omsi_o3d::Material {
        omsi_o3d::Material {
            diffuse: [1.0; 4],
            specular: [0.0; 3],
            emissive: [0.0; 3],
            specular_power: 0.0,
            texture: texture.into(),
        }
    }

    fn mesh(parts: &[(&[Vec3], u32)], textures: &[&str], head: f32) -> HumanMesh {
        let mut data = MeshData {
            one_sided: true,
            ..Default::default()
        };
        let mut tris: Vec<Tri> = Vec::new();
        for (points, slot) in parts {
            let base = data.positions.len() as u32;
            data.positions.extend_from_slice(points);
            for k in 1..points.len() as u32 - 1 {
                tris.push(([base, base + k, base + k + 1], *slot));
            }
        }
        data.normals = vec![Vec3::Y; data.positions.len()];
        data.uvs = vec![glam::Vec2::ZERO; data.positions.len()];
        for slot in 0..textures.len() as u32 {
            let first = data.indices.len() as u32;
            data.indices.extend(
                tris.iter()
                    .filter(|(_, s)| *s == slot)
                    .flat_map(|(t, _)| *t),
            );
            data.ranges
                .push((first, data.indices.len() as u32 - first, slot));
        }
        let skin = data
            .positions
            .iter()
            .map(|p| Influence {
                n: 1,
                slot: [if p.z > head { 2 } else { 1 }, 0, 0, 0],
                weight: [1.0, 0.0, 0.0, 0.0],
            })
            .collect();
        HumanMesh {
            bones: vec![(
                -12,
                (0..data.positions.len() as u32).map(|v| (v, 1.0)).collect(),
            )],
            data,
            materials: textures.iter().map(|t| material(t)).collect(),
            skin,
            alpha: vec![0; textures.len()],
        }
    }

    #[test]
    fn decimated_eyes_are_replaced_by_the_top_level_ones() {
        let body = [
            Vec3::new(-0.2, 0.0, 0.0),
            Vec3::new(0.2, 0.0, 0.0),
            Vec3::new(0.2, 0.0, 1.7),
            Vec3::new(-0.2, 0.0, 1.7),
        ];
        let eye = |x: f32| {
            (0..8)
                .map(|k| {
                    let a = k as f32 * std::f32::consts::TAU / 8.0;
                    Vec3::new(x + 0.012 * a.cos(), 0.1, 1.6 + 0.012 * a.sin())
                })
                .collect::<Vec<_>>()
        };
        let (left, right) = (eye(-0.03), eye(0.03));
        let top = mesh(
            &[(&body, 0), (&left, 1), (&right, 1)],
            &["skin.dds", "eye.dds"],
            1.5,
        );
        let remnant = [left[0], left[3], left[5]];
        let mut low = mesh(&[(&body, 0), (&remnant, 1)], &["skin.dds", "eye.dds"], 1.5);
        keep_small_parts(&mut low, &top);

        let tris = triangles(&low.data);
        assert_eq!(tris.iter().filter(|(_, s)| *s == 0).count(), 2);
        assert_eq!(tris.iter().filter(|(_, s)| *s == 1).count(), 12);
        let used: Vec<u32> = tris.iter().flat_map(|(t, _)| *t).collect();
        assert!(used.iter().all(|&i| i >= 7 || i < 4));
        assert_eq!(low.skin.len(), low.data.positions.len());
        assert!(low.skin[7..].iter().all(|s| s.slot[0] == 2));
        assert_eq!(low.bones[0].1.len(), low.data.positions.len());
        assert_eq!(low.materials.len(), 2);
    }

    #[test]
    fn a_missing_material_is_added_and_levels_without_small_parts_are_left_alone() {
        let body = [
            Vec3::new(-0.2, 0.0, 0.0),
            Vec3::new(0.2, 0.0, 0.0),
            Vec3::new(0.2, 0.0, 1.7),
            Vec3::new(-0.2, 0.0, 1.7),
        ];
        let lash = [
            Vec3::new(0.0, 0.1, 1.6),
            Vec3::new(0.02, 0.1, 1.6),
            Vec3::new(0.02, 0.1, 1.61),
        ];
        let top = mesh(&[(&body, 0), (&lash, 1)], &["skin.dds", "lash.dds"], 1.5);
        let mut low = mesh(&[(&body, 0), (&lash, 0)], &["skin.dds"], 1.5);
        keep_small_parts(&mut low, &top);
        assert_eq!(low.materials.len(), 2);
        assert_eq!(low.materials[1].texture, "lash.dds");
        assert_eq!(low.alpha, vec![0, 0]);
        let tris = triangles(&low.data);
        assert_eq!(tris.len(), 3);
        assert_eq!(tris.iter().filter(|(_, s)| *s == 1).count(), 1);

        let plain = mesh(&[(&body, 0)], &["skin.dds"], 1.5);
        for top in [&top, &plain] {
            let mut low = mesh(&[(&body, 0)], &["skin.dds"], 1.5);
            let before = low.data.indices.clone();
            keep_small_parts(&mut low, top);
            assert_eq!(low.data.indices, before);
            assert_eq!(low.materials.len(), 1);
        }
    }
}
