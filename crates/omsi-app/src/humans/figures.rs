use super::*;

/// `<group>/<name>~<other>.hum`: another figure for the place of `<group>/<name>.hum`.
pub(super) fn is_alternate(path: &Path) -> bool {
    path.file_stem()
        .is_some_and(|s| s.to_string_lossy().contains('~'))
}

pub(super) fn file_name(ty: &HumanType) -> std::borrow::Cow<'_, str> {
    ty.def
        .path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
}

pub(super) fn slot_key(path: &Path) -> String {
    let lower = |s: Option<&std::ffi::OsStr>| {
        s.map(|n| n.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default()
    };
    let group = lower(path.parent().and_then(|d| d.file_name()));
    let stem = lower(path.file_stem());
    format!("{group}/{}.hum", stem.split('~').next().unwrap_or_default())
}

pub(super) fn load_figures(
    root: &Path,
) -> (Vec<Arc<HumanType>>, HashMap<String, Vec<Arc<HumanType>>>) {
    let mut roots = omsi_cfg::content_dirs("Humans");
    if roots.is_empty() {
        roots.push(root.join("Humans"));
    }
    let mut found: Vec<(std::ffi::OsString, std::ffi::OsString, PathBuf)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for r in &roots {
        for (group, is_dir) in omsi_cfg::vfs::list_dir(r).unwrap_or_default() {
            if !is_dir {
                continue;
            }
            let d = r.join(&group);
            for (n, _) in omsi_cfg::vfs::list_dir(&d).unwrap_or_default() {
                let lower = n.to_string_lossy().to_ascii_lowercase();
                if lower.ends_with(".hum")
                    && !lower.contains("driver")
                    && seen.insert(format!(
                        "{}/{lower}",
                        group.to_string_lossy().to_ascii_lowercase()
                    ))
                {
                    found.push((group.clone(), n.clone(), d.join(&n)));
                }
            }
        }
    }
    found.sort();
    let (mut types, mut alternates) = (Vec::new(), HashMap::<String, Vec<_>>::new());
    for (_, _, f) in found {
        match HumanType::load(&f) {
            Ok(t) if is_alternate(&f) => alternates
                .entry(slot_key(&f))
                .or_default()
                .push(Arc::new(t)),
            Ok(t) => types.push(Arc::new(t)),
            Err(e) => log::warn!("human {}: {e:#}", f.display()),
        }
    }
    log::info!(
        "humans: {} types, {} alternates",
        types.len(),
        alternates.values().map(Vec::len).sum::<usize>()
    );
    (types, alternates)
}

/// An entry listed twice is drawn twice as often; each file is loaded once.
pub(super) fn map_human_types(root: &Path, list: &[String]) -> Vec<Arc<HumanType>> {
    let mut loaded: HashMap<String, Option<Arc<HumanType>>> = HashMap::new();
    let mut picked = Vec::new();
    for line in list {
        let rel = line.trim().replace('\\', "/");
        let rel = if rel.to_ascii_lowercase().starts_with("humans/") {
            rel
        } else {
            format!("Humans/{rel}")
        };
        let path = omsi_cfg::resolve_path(root, &rel);
        let ty = loaded
            .entry(path.to_string_lossy().to_lowercase())
            .or_insert_with(|| {
                HumanType::load(&path)
                    .map(Arc::new)
                    .map_err(|e| log::warn!("map human {}: {e:#}", path.display()))
                    .ok()
            });
        picked.extend(ty.clone());
    }
    picked
}

impl Humans {
    pub(super) fn use_map_humans(&mut self, world: &World) {
        if self.map_humans_done {
            return;
        }
        self.map_humans_done = true;
        let path = omsi_cfg::resolve_path(&world.map_dir, "humans.txt");
        let list = omsi_map::ailists::load_list(&path);
        if list.is_empty() {
            return;
        }
        let key = |p: &str| -> String {
            let p = p.replace('\\', "/").to_ascii_lowercase();
            match p.rfind("humans/") {
                Some(k) => p[k + 7..].to_string(),
                None => p,
            }
        };
        let mut picked: Vec<Arc<HumanType>> = Vec::new();
        for line in &list {
            let want = key(line.trim());
            match self
                .types
                .iter()
                .find(|t| key(&t.def.path.to_string_lossy()) == want)
            {
                Some(t) => picked.push(t.clone()),
                None => picked.extend(map_human_types(&world.root, std::slice::from_ref(line))),
            }
        }
        if picked.is_empty() {
            log::warn!("humans.txt of the map names nobody installed: keeping all people");
            return;
        }
        log::info!(
            "humans: {} of {} map entries loaded from {}",
            picked.len(),
            list.len(),
            path.display()
        );
        self.types = picked;
    }

    /// Not a twin of anybody near: a few tries for a figure nobody near wears, then other clothes.
    pub(super) fn pick_figure(
        &mut self,
        position: DVec3,
        kind: Option<usize>,
    ) -> (Arc<HumanType>, usize) {
        let near: Vec<(usize, usize)> = self
            .people
            .iter()
            .filter(|q| (q.position - position).truncate().length() < 30.0)
            .map(|q| (Arc::as_ptr(&q.ty) as usize, q.variant))
            .collect();
        let mut choice: Option<(Arc<HumanType>, usize)> = None;
        for attempt in 0..10 {
            let pick = (self.rand() % self.types.len() as u64) as usize;
            let mut t = self.types[kind.map(|k| k % self.types.len()).unwrap_or(pick)].clone();
            if let (None, Some(alts)) = (kind, self.alternates.get(&slot_key(&t.def.path))) {
                let weight = |t: &HumanType| t.def.weight.unwrap_or(1.0) as f64;
                let alts = alts.clone();
                let mut left = self.rand_f() * (1.0 + alts.iter().map(|a| weight(a)).sum::<f64>());
                for a in alts {
                    left -= weight(&a);
                    if left < 0.0 {
                        t = a;
                        break;
                    }
                }
            }
            let tk = Arc::as_ptr(&t) as usize;
            let n_var = t.variants.len() + 1;
            let v0 = (self.rand() % n_var as u64) as usize;
            let var = (0..n_var)
                .map(|k| (v0 + k) % n_var)
                .find(|v| !near.contains(&(tk, *v)));
            let figure_free = !near.iter().any(|n| n.0 == tk);
            match var {
                Some(v) if figure_free || attempt >= 6 || kind.is_some() => return (t, v),
                Some(v) if choice.is_none() => choice = Some((t, v)),
                None if choice.is_none() && attempt == 9 => choice = Some((t, v0)),
                _ => {}
            }
        }
        choice.unwrap_or_else(|| (self.types[0].clone(), 0))
    }

    pub fn type_index(&mut self, ty: Arc<HumanType>) -> usize {
        if let Some(i) = self
            .types
            .iter()
            .position(|t| Arc::ptr_eq(t, &ty) || t.def.path == ty.def.path)
        {
            return i;
        }
        self.types.push(ty);
        self.types.len() - 1
    }

    /// An alternate the host drew: that figure if it is here too, else the one whose place it took.
    pub fn type_by_file(&mut self, file: &str) -> Option<usize> {
        let want = file.replace('\\', "/").to_ascii_lowercase();
        let is = |t: &Arc<HumanType>, want: &str| {
            t.def
                .path
                .to_string_lossy()
                .replace('\\', "/")
                .to_ascii_lowercase()
                .ends_with(want)
        };
        if let Some(i) = self.types.iter().position(|t| is(t, &want)) {
            return Some(i);
        }
        let slot = slot_key(Path::new(&want));
        let alt = self
            .alternates
            .get(&slot)
            .and_then(|v| v.iter().find(|t| is(t, &want)).cloned());
        match alt {
            Some(t) => Some(self.type_index(t)),
            None if is_alternate(Path::new(&want)) => self.types.iter().position(|t| is(t, &slot)),
            None => None,
        }
    }

    pub fn type_file(ty: &HumanType) -> String {
        let p = ty.def.path.to_string_lossy().replace('\\', "/");
        match p.to_ascii_lowercase().rfind("/humans/") {
            Some(k) => p[k + 1..].to_string(),
            None => p,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alternates_share_the_place_of_their_type() {
        let alt = Path::new("C:/x/Humans/Other/Man01~Business_Male_01.hum");
        assert!(is_alternate(alt));
        assert!(!is_alternate(Path::new("C:/x/Humans/Other/man01.hum")));
        assert_eq!(slot_key(alt), "other/man01.hum");
        assert_eq!(
            slot_key(Path::new("Humans/Other/man01.hum")),
            "other/man01.hum"
        );
    }

    #[test]
    fn map_humans_load_nested_paths_and_preserve_weights() {
        let root =
            std::env::temp_dir().join(format!("omsi-map-human-paths-{}", std::process::id()));
        let nested = root.join("Humans/JP_Test/Child_1");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("Child_1.hum"), "[model]\nmodel.cfg\n").unwrap();
        std::fs::write(nested.join("model.cfg"), "").unwrap();
        let other = root.join("Humans/Other");
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(other.join("Man.hum"), "[model]\nmodel.cfg\n").unwrap();
        std::fs::write(other.join("model.cfg"), "").unwrap();
        let list = vec![
            "humans\\jp_test\\child_1\\child_1.hum".into(),
            "Humans/JP_Test/Child_1/Child_1.hum".into(),
            "JP_Test/Child_1/Child_1.hum".into(),
            "Humans/JP_Test/Missing.hum".into(),
        ];
        let picked = map_human_types(&root, &list);
        assert_eq!(picked.len(), 3);
        assert!(Arc::ptr_eq(&picked[0], &picked[1]));
        assert!(Arc::ptr_eq(&picked[1], &picked[2]));
        // on Windows `nested` keeps the slashes it was joined with
        let lower = |p: &Path| p.to_string_lossy().to_lowercase().replace('\\', "/");
        assert!(
            picked
                .iter()
                .all(|t| lower(&t.def.path).starts_with(&lower(&nested)))
        );
        assert!(map_human_types(&root, &["Humans/Missing/None.hum".into()]).is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
}
