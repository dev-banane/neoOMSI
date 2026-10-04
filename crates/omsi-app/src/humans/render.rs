use super::*;

pub(super) fn walk_input(speed: f32, dt: f32, natural: bool, seed: u32) -> AnimInput {
    AnimInput {
        kind: (speed > 0.05) as u8,
        speed,
        moved: speed * dt,
        room_height: OUTSIDE_ROOM,
        dt_ms: dt * 1000.0,
        natural,
        seed,
        ..Default::default()
    }
}

/// A soft dark patch, the sky light a body keeps off the floor under it.
fn contact_shadow(renderer: &Renderer, scene: &mut Scene) -> (MeshId, MaterialId) {
    const N: u32 = 64;
    let mut rgba = Vec::with_capacity((N * N * 4) as usize);
    let edge = (-4.5f32).exp();
    for y in 0..N {
        for x in 0..N {
            let u = (x as f32 + 0.5) / N as f32 * 2.0 - 1.0;
            let v = (y as f32 + 0.5) / N as f32 * 2.0 - 1.0;
            let r2 = u * u + v * v;
            let a = 0.6 * (((-4.5 * r2).exp() - edge) / (1.0 - edge)).max(0.0);
            rgba.extend_from_slice(&[0, 0, 0, (a * 255.0).round() as u8]);
        }
    }
    let tex = renderer.add_texture_data(
        scene,
        &omsi_texture::gpu::TextureData::from_image(omsi_texture::Image {
            width: N,
            height: N,
            rgba,
            has_alpha: true,
        }),
    );
    let corners = [(-0.5, -0.5), (0.5, -0.5), (0.5, 0.5), (-0.5, 0.5)];
    let mesh = omsi_geometry::MeshData {
        positions: corners.iter().map(|&(x, y)| Vec3::new(x, y, 0.0)).collect(),
        normals: vec![Vec3::Z; 4],
        uvs: corners
            .iter()
            .map(|&(x, y)| glam::Vec2::new(x + 0.5, 0.5 - y))
            .collect(),
        ranges: vec![(0, 6, 0)],
        indices: vec![0, 2, 1, 0, 3, 2],
        one_sided: false,
    };
    let mat = renderer.add_material(scene, Some(tex), AlphaMode::Blend, [1.0; 4], false);
    // drawn with the ground, before the bus: writing depth, it hid the bus floor under it
    renderer.set_no_z_write(scene, mat, true);
    (renderer.add_mesh(scene, &mesh), mat)
}

impl Humans {
    pub fn take_footfalls(&mut self) -> Vec<ambience::Footfall> {
        if self.footfalls.len() > 256 {
            self.footfalls.clear();
        }
        std::mem::take(&mut self.footfalls)
    }

    /// Only level 0 starts visible: `sync` shows the level it has posed.
    pub(super) fn add_meshes(
        &mut self,
        world: &World,
        renderer: &Renderer,
        scene: &mut Scene,
        ty: &HumanType,
        variant: usize,
        position: DVec3,
    ) -> Vec<(MeshId, usize)> {
        let tkey = ty as *const HumanType as usize;
        let mut meshes = Vec::new();
        for mi in 0..ty.mesh_count() {
            let (level, hm) = ty.mesh_at(mi);
            let key = (tkey, variant, mi);
            if let Some((id, inst)) = self.gpu.spare.get_mut(&key).and_then(|v| v.pop()) {
                self.gpu.hidden.retain(|h| *h != inst);
                renderer.set_transform(scene, inst, position, Mat4::IDENTITY);
                renderer.set_params(scene, inst, &[], level == 0, &[]);
                meshes.push((id, inst));
                continue;
            }
            if !self.gpu.materials.contains_key(&key) {
                let dirs = ty.texture_dirs(&world.root);
                let mut mats = Vec::new();
                for (k, m) in hm.materials.iter().enumerate() {
                    let (name, first) = ty.variant_texture(&m.texture, variant);
                    let mut look: Vec<&Path> = first.into_iter().collect();
                    look.extend(dirs.iter().map(|p| p.as_path()));
                    let found = omsi_texture::find_texture(name, &look)
                        .or_else(|| omsi_texture::find_texture(&m.texture, &look));
                    if found.is_none() && !m.texture.trim().is_empty() {
                        log::warn!(
                            "human {}: texture {} not found",
                            ty.def.path.display(),
                            m.texture
                        );
                    }
                    let tex = found.and_then(|path| {
                        *self.gpu.textures.entry(path.clone()).or_insert_with(|| {
                            let t = world
                                .textures
                                .get_gpu_fast(&path)
                                .map(|(img, _)| renderer.add_texture_data(scene, &img));
                            world.textures.release(&path);
                            t
                        })
                    });
                    let alpha = match hm.alpha.get(k).copied().unwrap_or(0) {
                        1 => AlphaMode::Test,
                        2 => AlphaMode::Blend,
                        _ => AlphaMode::Opaque,
                    };
                    mats.push(renderer.add_material(scene, tex, alpha, [1.0; 4], false));
                }
                self.gpu.materials.insert(key, mats);
            }
            let id = renderer.add_mesh(scene, &hm.data);
            let inst = renderer.add_instance(
                scene,
                id,
                position,
                Mat4::IDENTITY,
                self.gpu.materials[&key].clone(),
            );
            if level > 0 {
                renderer.set_params(scene, inst, &[], false, &[]);
            }
            meshes.push((id, inst));
        }
        meshes
    }

    pub(super) fn add_blob(&mut self, renderer: &Renderer, scene: &mut Scene, at: DVec3) -> usize {
        if let Some(inst) = self.gpu.spare_blobs.pop() {
            self.gpu.hidden.retain(|h| *h != inst);
            return inst;
        }
        let (mesh, mat) = *self
            .gpu
            .blob
            .get_or_insert_with(|| contact_shadow(renderer, scene));
        let inst = renderer.add_shadow_blob_instance(scene, mesh, at, Mat4::IDENTITY, vec![mat]);
        renderer.set_params(scene, inst, &[], false, &[]);
        inst
    }

    pub(super) fn retire(&mut self, p: &Person) {
        self.gpu.hidden.push(p.blob);
        self.gpu.spare_blobs.push(p.blob);
        let tkey = Arc::as_ptr(&p.ty) as usize;
        for (mi, m) in p.meshes.iter().enumerate() {
            self.gpu.hidden.push(m.1);
            self.gpu
                .spare
                .entry((tkey, p.variant, mi))
                .or_default()
                .push(*m);
        }
    }

    pub(super) fn animate(&mut self, dt: f32, f: &Frame) {
        for i in 0..self.people.len() {
            if self.people[i].avatar {
                self.animate_avatar(i, dt, f);
                continue;
            }
            let p = &self.people[i];
            let (natural, seed) = (self.natural, p.id);
            let (input, footstep) = match &p.state {
                State::Pax(x) => {
                    let bn = x.inside.and_then(|b| f.bus(Some(b)));
                    let own = |q: DVec3| -> Vec3 {
                        let v = q - x.pos;
                        let (s, c) = x.yaw.sin_cos();
                        omsi_sim::human_omsi::d3d(Vec3::new(
                            (v.x * c - v.y * s) as f32,
                            (v.x * s + v.y * c) as f32,
                            v.z as f32,
                        ))
                    };
                    let look = bn.filter(|_| x.look_driver).and_then(|b| {
                        b.cabin
                            .data
                            .driver_positions
                            .first()
                            .map(|d| own((Vec3::from(d.pos) + Vec3::Z * 0.65).as_dvec3()))
                    });
                    // the waiting watch the bus come in
                    let look = look.or_else(|| {
                        if !natural || x.task != Task::WaitingForBus || x.inside.is_some() {
                            return None;
                        }
                        f.buses()
                            .iter()
                            .filter(|b| b.speed.abs() > 0.5)
                            .map(|b| (b, (b.pos - x.pos).truncate().length()))
                            .filter(|(_, d)| *d < 70.0)
                            .min_by(|a, b| a.1.total_cmp(&b.1))
                            .map(|(b, _)| own(b.pos + DVec3::Z * 1.5))
                    });
                    let pack = bn.and_then(|b| {
                        x.step_pack
                            .and_then(|k| b.cabin.step_packs.get(k))
                            .map(|pk| (b.id, pk.clone()))
                    });
                    let input = AnimInput {
                        kind: x.pax_state.min(2),
                        speed: x.speed,
                        moved: x.moved,
                        room_height: x.room,
                        seat_height: x.seat_h,
                        reach: (x.reach && x.inside.is_some()).then(|| own(x.reach_at.as_dvec3())),
                        look,
                        smooth: x.smooth,
                        dt_ms: dt * 1000.0,
                        natural,
                        seed,
                    };
                    (input, pack)
                }
                _ => (walk_input(p.vel.length() as f32, dt, natural, seed), None),
            };
            let p = &mut self.people[i];
            let ev = p.anim.advance(&p.ty.omsi, &input);
            if let (true, Some((bus, pack))) = (ev.step, footstep) {
                self.footfalls.push(ambience::Footfall {
                    position: p.position,
                    inside: true,
                    own_bus: bus == BusId::Player,
                    pack: Some(pack),
                });
            }
        }
    }

    pub fn sync(&mut self, renderer: &Renderer, scene: &mut Scene, camera: DVec3) {
        for inst in self.gpu.hidden.drain(..) {
            renderer.set_params(scene, inst, &[], false, &[]);
        }
        let started = std::time::Instant::now();
        self.sync_frame = self.sync_frame.wrapping_add(1);
        let eye = self.eye;
        let from = eye.map(|e| e.pos).unwrap_or(camera);
        let all = self.time - self.last_sync > 0.12;
        let sdt = (self.time - self.last_sync).clamp(0.0, 0.5) as f32;
        self.last_sync = self.time;
        let fov_y = eye.map_or(1.0, |e| e.fov_y);
        let mut levels: Vec<(bool, usize)> = Vec::with_capacity(self.people.len());
        for (k, p) in self.people.iter_mut().enumerate() {
            p.since_posed = p.since_posed.saturating_add(1);
            let d = p.position + DVec3::Z * 0.9 - from;
            let dist = d.length();
            let level = if p.ty.levels.len() > 1 {
                let r = p.ty.radius() as f64;
                let od = (p.position - from).length();
                let size = if od <= r {
                    f32::MAX
                } else {
                    (2.0 * r / (od * fov_y)) as f32
                };
                p.ty.level_at(size, Some(p.level))
            } else {
                0
            };
            let visible =
                eye.is_none_or(|e| dist < 4.0 || d.dot(e.fwd) / dist.max(1e-3) > e.cos_half - 0.15);
            // the mirrors show the people behind the bus: within 30 m everybody every frame
            let every = match dist {
                d if d < 30.0 => 1,
                _ if !visible => 12,
                d if d < 45.0 => 1,
                d if d < 90.0 => 2,
                d if d < 160.0 => 3,
                _ => 6,
            };
            let every = if p.vel.length_squared() < 1e-4 && dist > 20.0 {
                every * 2
            } else {
                every
            };
            let turn = (self.sync_frame + k as u32) % every == 0;
            let due = !p.skinned
                || all
                || (1 << level) & !p.fresh_levels != 0
                || (p.since_posed >= every && (turn || p.since_posed >= 2 * every));
            levels.push((due, level));
        }
        let n_due = levels.iter().filter(|l| l.0).count();
        let pose = |(p, &(due, level)): (&mut Person, &(bool, usize))| {
            if due {
                pose_one(p, 1 << level);
            }
        };
        if n_due >= 8 {
            self.people
                .par_iter_mut()
                .zip(levels.par_iter())
                .with_min_len(2)
                .for_each(pose);
        } else {
            self.people.iter_mut().zip(levels.iter()).for_each(pose);
        }
        let upload = std::time::Instant::now();
        for (p, &(due, level)) in self.people.iter_mut().zip(&levels) {
            if due {
                for (k, (id, _)) in p.meshes.iter().enumerate() {
                    let (level, m) = p.ty.mesh_at(k);
                    if p.changed_levels & (1 << level) != 0 {
                        if let Some((pos, nrm)) = p.skins.get(k) {
                            renderer.update_mesh(scene, *id, pos, nrm, &m.data.uvs);
                        }
                    }
                }
                p.skinned = true;
                p.since_posed = 0;
            }
            let hidden = self.avatars.hidden.get(&p.id).copied();
            let switch = level != p.level && p.fresh_levels & (1 << level) != 0;
            if switch {
                p.level = level;
            }
            // a hidden avatar is set every frame: the posing would show it again
            if switch || hidden.is_some() {
                for (k, (_, inst)) in p.meshes.iter().enumerate() {
                    let shown = p.ty.mesh_at(k).0 == p.level && hidden != Some(true);
                    renderer.set_params(scene, *inst, &[], shown, &[]);
                }
            }
            let inside = matches!(p.place, Place::Bus(..));
            let tilt = if inside { p.tilt } else { Mat4::IDENTITY };
            let xf = tilt * Mat4::from_rotation_z((-p.heading).to_radians() as f32);
            let lit_to = if inside { p.interior } else { 0.0 };
            p.lit += (lit_to - p.lit) * (sdt / 0.4).min(1.0);
            for (_, inst) in &p.meshes {
                renderer.set_transform(scene, *inst, p.position, xf);
                renderer.set_interior(scene, *inst, p.lit * 0.5);
            }
            let seated = p.anim.angles[0].abs() >= 45.0;
            let blob = hidden != Some(true) && !seated && (p.position - from).length() < 90.0;
            if blob != p.blob_shown {
                p.blob_shown = blob;
                renderer.set_params(scene, p.blob, &[], blob, &[]);
            }
            if blob {
                let size = (p.ty.def.height.clamp(0.9, 2.1) / 1.75) * 0.6;
                let stretch = 1.0 + 0.3 * (p.vel.length() as f32 / 1.5).min(1.0);
                let scale = Mat4::from_scale(Vec3::new(size, size * 1.15 * stretch, 1.0));
                renderer.set_transform(scene, p.blob, p.position + DVec3::Z * 0.01, xf * scale);
            }
        }
        self.pose_stats.0 += 1;
        self.pose_stats.1 += n_due;
        self.pose_stats.2 += started.elapsed().as_secs_f64() * 1000.0;
        self.pose_stats.3 += upload.elapsed().as_secs_f64() * 1000.0;
    }
}

fn pose_one(p: &mut Person, want: u32) {
    let Person {
        anim,
        ty,
        skins,
        skin_bones,
        fresh_levels,
        changed_levels,
        ..
    } = p;
    *changed_levels = 0;
    // seated the thighs are well forward: the feet stay where the seat puts them
    let standing = anim.angles[0].abs() < 45.0 && anim.angles[1].abs() < 45.0;
    let bones = omsi_sim::human::slots_from_omsi(&anim.bones(&ty.omsi), &ty.rig, standing);
    if bones.iter().any(|b| !b.is_finite()) && !skins.is_empty() {
        return;
    }
    let same = skins.len() == ty.mesh_count()
        && skin_bones
            .as_ref()
            .is_some_and(|b| b.iter().zip(&bones).all(|(a, c)| a.abs_diff_eq(*c, 1e-6)));
    let todo = if same { want & !*fresh_levels } else { want };
    if todo == 0 {
        return;
    }
    skins.resize_with(ty.mesh_count(), Default::default);
    for (k, skinned) in skins.iter_mut().enumerate() {
        let (level, m) = ty.mesh_at(k);
        if todo & (1 << level) != 0 {
            skin(m, &bones, &mut skinned.0, &mut skinned.1);
        }
    }
    *fresh_levels = if same { *fresh_levels | todo } else { todo };
    *skin_bones = Some(bones);
    *changed_levels = todo;
}
