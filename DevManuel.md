# Manuel développeur - Goard

---

## 1) Stack technique

| Composant | Bibliothèque / Version |
|-----------|----------------------|
| UI framework | [egui](https://github.com/emilk/egui) 0.30 + eframe 0.30 |
| Graphes | egui_plot 0.30 |
| Sérialisation | serde 1.0 + serde_json 1.0 |
| Dates | chrono 0.4 (+ chrono-tz) |
| i18n | rust-i18n 3 |
| WASM | wasm-bindgen, web-sys, js-sys |
| Cibles | native (Linux/macOS/Windows) + `wasm32-unknown-unknown` |

---

## 2) Structure du projet

```
src/
├── main.rs              — point d'entrée, bootstrap native + WASM
├── app.rs               — boucle principale egui (App::update)
├── file_import.rs       — boîte de dialogue fichier (native: rfd, WASM: <input>)
│
├── models/
│   ├── data_structure/
│   │   ├── application_context.rs  — état central (voir §3)
│   │   ├── refresh_coordinator.rs  — channels MPSC + mutex partagés
│   │   ├── import_state.rs         — onglets importés + groupes
│   │   ├── ui_preferences.rs       — préférences UI + presets clusters
│   │   ├── live_data_state.rs      — données jobs/clusters/strata en mémoire
│   │   ├── gantt_config.rs         — config.json (couleurs, timespan)
│   │   ├── job.rs / cluster.rs / host.rs / cpu.rs / resource.rs / strata.rs
│   │   ├── filters.rs              — état des filtres jobs
│   │   └── marker.rs               — annotations ponctuelles sur le Gantt
│   │
│   ├── file_types/
│   │   ├── mod.rs          — trait FileTypeConfig + FileTypeRegistry
│   │   ├── oar.rs          — type OAR Simulation
│   │   ├── energy_series.rs — type Energy Series
│   │   └── event.rs        — type Event
│   │
│   └── utils/
│       ├── updater.rs      — update_periodically / instant_update
│       ├── parser.rs       — lecture data.json (jobs, resources, dead intervals)
│       ├── mocker.rs       — données factices pour WASM (pas de SSH)
│       ├── date_converter.rs
│       └── utils.rs        — helpers clusters/hosts/resources
│
├── views/
│   ├── menu/
│   │   ├── menu.rs         — barre de menu (Fichier, Options, ?)
│   │   ├── tools.rs        — barre d'outils + ligne de synthèse Gantt
│   │   ├── filtering.rs    — panneau Filtres
│   │   └── options.rs      — panneau Options (langue, police, thème)
│   │
│   └── main_page/
│       ├── dashboard.rs    — vue Dashboard
│       ├── gantt/
│       │   ├── mod.rs      — GanttChart : onglets, panels, rendu principal
│       │   ├── canvas.rs   — dessin des lignes de ressources + jobs
│       │   ├── interaction.rs — gestion zoom/pan souris/clavier
│       │   ├── timeline.rs — axe temporel + ligne "maintenant"
│       │   ├── labels.rs   — étiquettes gutter
│       │   ├── jobs.rs     — résolution champs strata, tri ressources
│       │   ├── panels.rs   — panneaux Admin, Create/Edit view, Create/Edit preset, Energy
│       │   ├── energy_plot.rs — dessin du graphe énergie (egui_plot)
│       │   ├── energy_estimate.rs — estimation puissance depuis jobs
│       │   ├── theme.rs    — couleurs selon thème clair/sombre
│       │   └── types.rs    — Options, Info, ResourceFilter, LeafInfoPreset
│       └── anthentification.rs — formulaire login
│
file_types/          — configs JSON des types de fichiers (embarquées à la compilation)
├── oar.json
├── energy_series.json
└── event.json

config.json          — config Gantt (couleurs état, timespan)
views.json           — vues Gantt sauvegardées + leaf info presets
presets.json         — presets de clusters
options.json         — préférences utilisateur (police, langue)
```

---

## 3) Architecture de l'état (`ApplicationContext`)

`ApplicationContext` est le conteneur central. Il est découpé en quatre sous-structs :

| Champ | Type | Contenu |
|-------|------|---------|
| `data` | `LiveDataState` | jobs, clusters, strata (actifs + en attente swap) |
| `refresh` | `RefreshCoordinator` | channels MPSC, mutex dates/rate/flag |
| `import` | `ImportState` | sources importées, onglet actif, groupes |
| `prefs` | `UiPreferences` | police, thème, presets clusters, état vue Gantt |

L'état de session (`view_type`, `user_connected`, `filters`, `live_data`) reste à plat sur `ApplicationContext` car il est utilisé partout.

### Swap pattern (live data)

Le thread background écrit dans `swap_all_jobs` / `swap_all_clusters`. Le thread UI les copie dans `all_jobs` / `all_clusters` uniquement quand `check_data_update()` consomme le channel. Cela évite une lecture partielle d'un snapshot en cours d'écriture.

---

## 4) Mode live data

Activé par le flag `--live` au lancement, ou via **Fichier → 📡 Live Data** depuis l'interface.

### Flux

```
App::new(live=true)
  └── ApplicationContext::update_periodically()
        └── thread::spawn ──► loop:
              1. Attendre refresh_rate secondes
              2. get_current_jobs_for_period(start, end)   ← SSH + parsing
              3. get_jobs_from_json("./data/data.json")
              4. jobs_sender.send(jobs)
              5. resources_sender.send(resources)
              6. dead_intervals_sender.send(intervals)

App::update() chaque frame :
  └── check_data_update()
        ├── jobs_receiver.try_recv()       → swap_all_jobs
        ├── resources_receiver.try_recv()  → rebuild clusters + strata
        └── dead_intervals_receiver.try_recv()
```

### Mutex partagés (`RefreshCoordinator`)

| Mutex | Rôle |
|-------|------|
| `refresh_rate: Arc<Mutex<u64>>` | secondes entre rafraîchissements (u64::MAX = jamais) |
| `is_refreshing: Arc<Mutex<bool>>` | verrou anti-doublons |
| `start_date / end_date` | fenêtre temporelle visible (mise à jour quand l'utilisateur pan le Gantt) |

### WASM

Sans accès SSH, `update_periodically()` et `instant_update()` utilisent `mocker.rs` qui génère des données factices. Pas de thread background en WASM (utiliser `wasm_bindgen_futures::spawn_local` si besoin à l'avenir).

---

## 5) Système de types de fichiers

### Trait `FileTypeConfig` (`src/models/file_types/mod.rs`)

```rust
pub trait FileTypeConfig: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn visualization_targets(&self) -> &[VisualizationTarget];
    fn detect(&self, content: &str) -> f32;      // confiance 0.0–1.0
    fn validate(&self, content: &str) -> Vec<ValidationError>;
    fn parse(&self, content: &str) -> Result<ParsedFileData, String>;

    // Optionnel — surcharger si nécessaire :
    fn supports_hierarchy_controls(&self) -> bool { true }
    fn hierarchy_levels(&self) -> Option<Vec<String>> { None }
}
```

`ParsedFileData` contient : `resources`, `clusters`, `jobs`, `strata_by_resource_id`, `raw_energy_series`, `markers`.

### `FileTypeRegistry`

L'ordre d'enregistrement définit la priorité en cas d'égalité de score `detect`. Actuellement :

```rust
// src/models/file_types/mod.rs — impl Default for FileTypeRegistry
registry.register(Box::new(oar::OarFileType::new()));
registry.register(Box::new(energy_series::EnergySeriesFileType::new()));
registry.register(Box::new(event::EventFileType::new()));
```

### Ajouter un nouveau type de fichier

**Étape 1 — Créer `src/models/file_types/mytype.rs`**

```rust
use super::{FileTypeConfig, ParsedFileData, ValidationError, VisualizationTarget};

pub struct MyFileType;

impl MyFileType {
    pub fn new() -> Self { Self }
}

impl FileTypeConfig for MyFileType {
    fn name(&self) -> &str { "My Type" }

    fn description(&self) -> &str { "Description affichée dans la boîte de dialogue import" }

    fn visualization_targets(&self) -> &[VisualizationTarget] {
        // Choisir parmi : Gantt, EnergyDiagram (ou les deux)
        &[VisualizationTarget::Gantt]
    }

    fn detect(&self, content: &str) -> f32 {
        // Retourner une valeur > 0.5 si ce fichier correspond au format.
        // Plus le score est élevé, plus ce type est prioritaire en auto-détection.
        let Ok(val) = serde_json::from_str::<serde_json::Value>(content) else { return 0.0 };
        if val.get("my_required_field").is_some() { 0.9 } else { 0.0 }
    }

    fn validate(&self, content: &str) -> Vec<ValidationError> {
        // Retourner une liste vide si le contenu est valide.
        Vec::new()
    }

    fn parse(&self, content: &str) -> Result<ParsedFileData, String> {
        // Parser le JSON et remplir ParsedFileData.
        // - jobs       : Vec<Job>       pour Gantt
        // - resources  : Vec<Strata>    pour les lignes du Gantt
        // - clusters   : Vec<Cluster>   optionnel
        // - raw_energy_series : Option<Vec<(i64, f64)>>  pour graphe énergie
        // - markers    : Vec<GanttMarker> pour cercles/annotations
        Ok(ParsedFileData {
            resources: Vec::new(),
            clusters: Vec::new(),
            jobs: Vec::new(),
            strata_by_resource_id: Default::default(),
            raw_energy_series: None,
            markers: Vec::new(),
        })
    }

    // Optionnel : si le type impose sa propre hiérarchie (pas celle des vues OAR)
    fn supports_hierarchy_controls(&self) -> bool { false }
    fn hierarchy_levels(&self) -> Option<Vec<String>> {
        Some(vec!["my_level".to_string()])
    }
}
```

**Étape 2 — Déclarer le module**

Dans `src/models/file_types/mod.rs` :

```rust
pub mod mytype;
```

**Étape 3 — Enregistrer dans la registry**

Dans `impl Default for FileTypeRegistry` (même fichier) :

```rust
registry.register(Box::new(mytype::MyFileType::new()));
```

L'ordre = priorité en cas d'égalité de score `detect`. Placer les types les plus spécifiques en premier.

---

## 6) Vues Gantt — édition directe de `views.json`

Le fichier `views.json` est chargé au démarrage et réécrit à chaque modification via l'interface Admin. Il peut aussi être édité manuellement à froid.

### Format complet

```json
{
  "views": [
    {
      "name": "Nodes",
      "levels": ["site", "cluster", "host"],
      "leaf_label_template": "{host|short}",
      "sort_by_label": false,
      "summary_fields": ["cluster", "host"],
      "leaf_infos": "host_info",
      "filter": {
        "field": "production",
        "value": "YES",
        "exclude": false
      }
    }
  ],
  "leaf_info_presets": [
    {
      "id": "host_info",
      "name": "Host",
      "fields": ["network_address", "comment", "cputype", "cpuset", "nodeset"]
    }
  ]
}
```

### Champs d'une vue

| Champ | Type | Obligatoire | Description |
|-------|------|-------------|-------------|
| `name` | string | oui | Nom affiché dans le menu View |
| `levels` | string[] | oui | Niveaux hiérarchiques, du plus général au plus fin |
| `leaf_label_template` | string \| null | non | Template d'étiquette. Variables : `{field}` ou `{field\|short}` (coupe avant le premier `.`) |
| `sort_by_label` | bool | non (défaut: false) | Trier les groupes par étiquette calculée au lieu de la clé brute |
| `summary_fields` | string[] | non | Champs affichés dans la barre de synthèse. Si vide : dernier niveau |
| `leaf_infos` | string \| null | non | `id` d'un preset `leaf_info_presets` |
| `filter` | object \| null | non | Filtre sur un champ de ressource (voir ci-dessous) |

### Champs du filtre

```json
{
  "field": "production",
  "value": "YES",
  "exclude": false
}
```

- `exclude: false` → liste blanche (garder seulement `field == value`)
- `exclude: true` → liste noire (exclure quand `field == value`)

### Champs d'un leaf_info_preset

| Champ | Description |
|-------|-------------|
| `id` | Identifiant unique, référencé par `leaf_infos` dans les vues |
| `name` | Label affiché en tête du tooltip |
| `fields` | Liste des champs strata à afficher dans le tooltip |

### Niveaux disponibles (champs strata)

Les valeurs utilisables dans `levels`, `summary_fields`, et `fields` sont les champs de la struct `Strata`. Les plus courants :

`site`, `cluster`, `host`, `type`, `vlan`, `disk`, `disk_id`, `nodeset`, `subnet_address`, `subnet_prefix`, `slash_16` … `slash_22`, `network_address`, `ip`, `comment`, `nodemodel`, `cputype`, `cpufreq`, `core_count`, `thread_count`, `memnode`, `gpu_model`, `chassis`, `resource_id`, `production`, `state`, `besteffort`, `deploy`, `drain`

Le champ `site` est dérivé automatiquement depuis le FQDN du host (première partie avant le premier composant court).

---

## 7) Fichiers de configuration

### `config.json`

Chargé par `GanttConfig::load()` au démarrage. Embarqué dans le binaire WASM à la compilation.

```json
{
  "standby_truncate_state_to_now": true,
  "besteffort_truncate_job_to_now": true,
  "min_state_duration": 2,
  "default_timespan": 21600,
  "state_colors": {
    "Absent": "#1e64dc",
    "Suspected": "#dc1e1e",
    "Dead": "#787878",
    "Standby": "#88ffff"
  },
  "state_colors_light": {
    "Absent": "#1040a0",
    "Suspected": "#a01010",
    "Dead": "#404040",
    "Standby": "#008888"
  }
}
```

| Clé | Effet |
|-----|-------|
| `standby_truncate_state_to_now` | Tronque les intervalles Absent en cours au moment présent quand Standby s'applique |
| `besteffort_truncate_job_to_now` | Cache la portion future des jobs besteffort |
| `min_state_duration` | Durée minimale (secondes) pour afficher un intervalle d'état |
| `default_timespan` | Largeur initiale du Gantt en secondes (21600 = 6h) |
| `job_color_min` | Composante RGB minimale pour les couleurs de jobs aléatoires (0–255). 0 = spectre complet, 255 = blanc. Défaut 140 : couleurs claires, labels noirs lisibles. |
| `state_colors` | Couleurs hatch mode sombre |
| `state_colors_light` | Couleurs hatch mode clair |

### `views.json`

Vues Gantt + leaf info presets. Écrit par l'interface Admin, éditable manuellement (voir §6).

### `presets.json`

Presets de clusters. Format :

```json
[
  { "name": "Mon preset", "clusters": ["cluster-a", "cluster-b"] }
]
```

### `options.json`

Préférences utilisateur. Écrit par le panneau Options.

```json
{ "font_size": 16, "language": "fr" }
```

---

## 8) Authentification

L'authentification est une **preuve de concept** : identifiants codés en dur (`admin` / `admin`) dans `src/views/main_page/anthentification.rs`.

Le flag `is_admin()` sur `ApplicationContext` vérifie `user_connected == Some("admin")`.

Les fonctions protégées sont : création/édition/suppression de vues Gantt, création/édition/suppression de presets de clusters.

**Ne pas déployer en production sans remplacer ce mécanisme.**

---

## 9) Cache de préférences par onglet (`tab_states.json`)

### Principe

Chaque fichier importé dispose d'un identifiant stable composé de deux clés :

| Clé | Calcul | Rôle |
|-----|--------|------|
| **Chemin absolu** | canonicalisé à l'import (`std::fs::canonicalize`) | Lookup rapide O(1) |
| **Hash FNV-1a 64 bits** | premiers 8 Ko du contenu + longueur totale | Fallback si fichier déplacé/renommé |

À l'ouverture d'un onglet, le cache est interrogé d'abord par chemin, puis par hash. Si une correspondance est trouvée, les préférences sont restaurées immédiatement.

### Données persistées par onglet

| Champ | Description |
|-------|-------------|
| `canvas_width_s` | Largeur visible du Gantt en secondes (niveau de zoom) |
| `sideways_pan` | Décalage horizontal en points |
| `row_height` | Hauteur des lignes de ressources |
| `view_index` | Index de la vue d'agrégation active |
| `energy_y_min/max` | Bornes Y du graphe énergie |
| `energy_fit` | Case « Ajuster à la figure » |
| `energy_panel_height` | Hauteur du panneau énergie |

### Déclencheurs de sauvegarde

| Événement | Code |
|-----------|------|
| Changement d'onglet | Blocs save dans `render_compact_toolbar` et `render` |
| Fermeture d'un onglet | `close_ds` handler dans `render_data_source_tabs` (via `persist_tab_state`) |
| Quitter l'application | `eframe::App::on_exit` → `flush_all_tab_states` |

> **Règle importante :** pour l'onglet actuellement actif, `persist_tab_state` lit directement `self.options.*` et `self.energy.*` (état courant) et non le `tab_view_state` HashMap (snapshot potentiellement périmé). Pour les onglets en arrière-plan, il lit le HashMap mis à jour lors du dernier départ.

### Fichiers et structs concernés

| Fichier | Rôle |
|---------|------|
| `src/models/data_structure/tab_state_cache.rs` | `TabStateCache` (load/save/lookup/store) + `compute_file_hash()` |
| `src/models/data_structure/import_state.rs` | `ImportedDataSource.file_hash: Option<String>` |
| `src/models/data_structure/application_context.rs` | Calcul du hash + canonicalisation du chemin à l'import |
| `src/views/main_page/gantt/mod.rs` | `GanttChart::persist_tab_state`, `flush_all_tab_states`, `restore_from_cache` |
| `src/app.rs` | `on_exit` → `flush_all_tab_states` |

### Fichier de cache

`tab_states.json` est écrit dans le répertoire de travail courant (là où le binaire est lancé). Il est **gitignore** : ne jamais le committer avec des chemins machine-spécifiques.

Format d'une entrée :

```json
{
  "path": "/chemin/absolu/vers/fichier.json",
  "hash": "494729c9f071c0bc",
  "state": {
    "canvas_width_s": 86400.0,
    "sideways_pan": 0.0,
    "row_height": 20.0,
    "view_index": 0,
    "energy_y_min": null,
    "energy_y_max": null,
    "energy_fit": true,
    "energy_panel_height": 270.0
  }
}
```

Maximum 200 entrées (FIFO). La clé de déduplication est le hash (pas le chemin) : si un fichier est déplacé, le chemin stocké est mis à jour automatiquement à la prochaine ouverture.

---

## 10) Diagramme énergie

### Sources de données

| Situation | Série affichée |
|-----------|---------------|
| Aucun fichier Energy Series | Estimation depuis les jobs (wattage TDP approximatif par CPU) |
| Fichier Energy Series seul | Courbe mesurée brute |
| Groupe OAR + Energy Series | Courbe estimée (OAR) + courbe mesurée (Energy Series) superposées |

La logique d'estimation est dans `src/views/main_page/gantt/energy_estimate.rs`.

### Synchronisation Gantt ↔ énergie

Quand l'utilisateur glisse dans le graphe énergie, `EnergyPanelState::show()` retourne un `Option<(i64, i64)>` (nouvelle plage visible). `GanttChart::render()` applique ce range en mettant à jour `options.canvas_width_s` et `options.sideways_pan_in_points`.

Le séparateur entre les deux zones est glissable verticalement ; `EnergyPanelState.panel_height` est mis à jour par le delta de drag.
