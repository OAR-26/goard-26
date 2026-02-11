# Refactor du Gantt (février 2026)

## Objectif

Le fichier du Gantt était très long et mélangeait plusieurs responsabilités (UI + interaction + agrégation + rendu + timeline).
Ce refactor découpe le code en sous-modules *sans changer le comportement* afin de :

- améliorer la lisibilité et la navigabilité
- réduire le risque de régressions lors d’évolutions
- limiter la duplication et quelques `clone()` coûteux

## Comprendre les changements (version simple)

### 1) Le "gros" fichier a été remplacé par un module dossier

Avant, tout était dans **un seul fichier** :

- `views/main_page/gantt.rs`

Après, le Gantt est devenu **un dossier module** (avec plusieurs fichiers), dont le point d’entrée est :

- `views/main_page/gantt/mod.rs`

Pourquoi c’est important en Rust :

- Si `views/main_page/gantt.rs` existe, il **prend le dessus** sur `views/main_page/gantt/mod.rs`.
- Donc il fallait supprimer l’ancien `gantt.rs`, sinon le compilateur continuerait d’utiliser l’ancienne version.

### 2) Le comportement ne change pas, seulement la structure

L’API externe reste la même :

- on continue d’utiliser `crate::views::main_page::gantt::GanttChart`

Le code est juste réparti par responsabilité, ce qui rend le tout plus facile à maintenir.

### 3) Un schéma pour comprendre le flux (qui appelle qui)

Quand l’écran Gantt est affiché, l’ordre global est :

1. `GanttChart::render` (dans `gantt/mod.rs`)
2. gestion pan/zoom : `interaction::interact_with_canvas` (dans `gantt/interaction.rs`)
3. rendu principal : `canvas::ui_canvas` (dans `gantt/canvas.rs`)
4. peinture des jobs/labels : fonctions dans `gantt/jobs.rs`
5. peinture timeline + ligne du temps courant : fonctions dans `gantt/timeline.rs`

## Résultat principal

- L’ancien module `views/main_page/gantt.rs` a été remplacé par un module dossier :
  `views/main_page/gantt/` (module racine : `views/main_page/gantt/mod.rs`).
- L’API externe ne change pas : on continue d’utiliser `crate::views::main_page::gantt::GanttChart`.

## Nouvelle organisation des fichiers

Le Gantt est maintenant organisé ainsi :

- `views/main_page/gantt/mod.rs`
  - Point d’entrée du module.
  - Contient `pub struct GanttChart` et l’implémentation de `View::render`.
  - Orchestre : menu/settings, création du canvas egui, et appels aux sous-modules.

- `views/main_page/gantt/types.rs`
  - Types partagés : `Info`, `Options`, constantes (`GUTTER_WIDTH`).

- `views/main_page/gantt/interaction.rs`
  - Gestion des interactions : pan/zoom, double-clic reset, animation de zoom.

- `views/main_page/gantt/canvas.rs`
  - « Contrôleur » du rendu :
    - prépare les structures d’agrégation (maps)
    - appelle les painters niveau 1/niveau 2
    - gère la séquence d’effets de frame (tooltip, hovered job, timeline text)

- `views/main_page/gantt/jobs.rs`
  - Peinture des jobs/labels (gutter) et tooltip.
  - `paint_job`, `paint_job_info`, `paint_tooltip`, et les painters d’agrégation.

- `views/main_page/gantt/timeline.rs`
  - Peinture de la timeline, labels et ligne « temps courant ».

- `views/main_page/gantt/theme.rs`
  - Couleurs et thèmes (dark/light) : `ThemeColors`, `get_theme_colors`.

- `views/main_page/gantt/labels.rs`
  - Helpers liés aux labels : `short_host_label`, `site_for_cluster_name`, `LabelMeta`, etc.

## Changements techniques (détaillés mais concrets)

### A) Agrégation : moins de copies (`clone`) inutiles

Le Gantt construit des groupes (par owner/host/cluster…) avant de peindre.

- Avant : on faisait des maps contenant des **copies** de jobs :
  - `BTreeMap<String, Vec<Job>>`
  - et dans les boucles : `push(job.clone())`

- Après : pendant la phase de regroupement, on stocke des **références** vers les jobs existants :
  - `BTreeMap<String, Vec<&Job>>`

Ce que ça change :

- On évite de recopier potentiellement des centaines/milliers de jobs juste pour les classer.
- Le comportement visuel reste identique.

Important : on garde des `clone()` là où c’est nécessaire.
Exemples :

- Quand on sauvegarde le job dans `options.current_hovered_job` (il faut une valeur possédée, pas une référence temporaire).
- Quand on ouvre une fenêtre de détails (`JobDetailsWindow::new(job.clone(), ...)`).

### B) Séparation des responsabilités (pour éviter l’effet "spaghetti")

Avant, une modification du tooltip ou de la timeline risquait d’impacter l’agrégation et l’inverse, car tout était mêlé.

Après :

- Interaction (zoom/pan) = `interaction.rs`
- Timeline = `timeline.rs`
- Couleurs (dark/light) = `theme.rs`
- Labels (host short, site/cluster) = `labels.rs`
- Rendu jobs + tooltip = `jobs.rs`
- Routing global/agrégation/choix des painters = `canvas.rs`

Ce découpage rend les changements plus sûrs : quand vous touchez la timeline, vous ne touchez pas au code de grouping.

## Vérification

Pour vérifier que le refactor ne casse rien :

- `cargo check`
- `cargo run`

## Pistes d’amélioration (optionnel)

Ce refactor est volontairement conservateur (objectif : réduire la taille du fichier et clarifier l’architecture, sans changer le comportement).
Si vous voulez aller plus loin ensuite :

- réduire les `unwrap()` (éviter les panic en cas de données inattendues)
- factoriser la logique d’agrégation (niveau1/niveau2) via des helpers “group_by”
- centraliser les constantes UI/magic numbers dans un bloc `const`
