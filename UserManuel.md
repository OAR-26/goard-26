# Manuel utilisateur - Goard

Ce manuel décrit l'utilisation de l'application **Goard** et l'ensemble des fonctionnalités visibles dans l'interface.

---

## 1) Démarrage

Au lancement, l'application s'ouvre directement sur la vue **Gantt**.

L'authentification n'est **pas requise** pour consulter les données. Elle est uniquement nécessaire pour les fonctions d'administration :
- Créer / modifier / supprimer des vues Gantt
- Créer / modifier / supprimer des presets de clusters

### Identifiants administrateur
- **Utilisateur** : `admin`
- **Mot de passe** : `admin`

> Note : l'authentification actuelle est une preuve de concept (identifiants codés en dur).

---

## 2) Barre de menu (haut)

### Fichier
- **📁 Import File** : ouvre une boîte de dialogue pour importer un fichier JSON (voir section 10)
- **📡 Live Data** : active le mode données en temps réel (désactivé si déjà actif)
- **Se connecter** (si non connecté) / **Se déconnecter** (si connecté)
- Affichage de l'utilisateur connecté
- **Quitter** : ferme l'application

### Options
Ouvre la fenêtre d'options pour :
- **Langue** : English / Français
- **Taille de police** : de 10 à 30
- **Enregistrer** : sauvegarde dans `options.json`

### Aide contextuelle (`?`)
Affiche une aide différente selon la vue active (Dashboard ou Gantt).

---

## 3) Barre d'outils (sous le menu)

Fonctionnalités globales :
- **Mode** : bouton `📊 Dashboard` / `📅 Gantt`
- **Filtres** : bouton `🔎 Filtres`
- **Thème clair/sombre** : bouton `☀` / `🌙`

Contrôles disponibles **uniquement en mode Live Data** :
- **Rafraîchissement automatique** : choix `30 s`, `1 min`, `5 min`, `Never`
- **Rafraîchissement immédiat** : bouton `⟳` (désactivé pendant un rafraîchissement en cours)

Comportement :
- Un indicateur en bas (`Refreshing data...`) + spinner apparaît pendant l'actualisation

---

## 4) Filtres des jobs

La fenêtre **Filtres** permet de filtrer l'affichage par :
- **Propriétaire (Owner)**
- **État du job (State)**
- **Preset de clusters** (None ou preset nommé)

Boutons :
- **Appliquer** : applique les filtres et met à jour l'affichage
- **Réinitialiser** : remet les filtres par défaut

Les filtres impactent :
- le Dashboard (métriques + tableau)
- le Gantt
- le calcul énergétique (dans la vue Gantt)

---

## 5) Vue Gantt

La vue Gantt affiche les jobs et ressources sur une timeline interactive.

### Onglets de sources de données

En haut du Gantt, une rangée d'onglets permet de naviguer entre les sources :

- **Live Data** : données temps réel (visible uniquement si le mode live est actif)
  - `×` à droite : désactive le mode live et vide les données en mémoire
- **Fichiers importés** : un onglet par fichier chargé
  - `+` : groupe ce fichier avec un autre (voir section 11)
  - `×` : ferme et supprime la source

### Interactions principales
- **Glisser (clic gauche)** : déplacement horizontal
- **Zoom horizontal** : `Ctrl/Cmd + molette` ou glisser vertical clic droit
- **Zoom vertical** : `Alt/Option + molette`
- **Double clic gauche** : réinitialiser la vue
- **Clic gauche sur un job** : zoom sur le job
- **Clic droit sur un job** : ouvrir les détails

### Contrôles Gantt (barre outils)
- **View** : sélecteur de vue d'agrégation (voir section 7)
- `🔧 Settings`
  - Couleur des jobs (aléatoire / par état)
- **Admin** : accès au panneau d'administration (grisé si non authentifié)
- **Nav** : `◀ 1w`, `◀ 1d`, `1d ▶`, `1w ▶`
- `⌚ Center on now`

### Ligne de synthèse (en mode Gantt)
Affiche :
- Nom de la vue active
- Nombre de jobs filtrés
- Champs de résumé configurés (ex. clusters affichés / total, hosts affichés / total)
- État des données (`refreshing`, `loading`, `ready`)

### Détails job
Les fenêtres de détails restent ouvertes individuellement et peuvent être fermées séparément.

---

## 6) Énergie (vue Gantt)

Sous le Gantt, un graphe **Consommation globale** est affiché.

Deux modes d'affichage :
- **Estimée** : calculée à partir des données de jobs (aucun fichier énergie chargé)
- **Mesurée** : issue d'un fichier `Energy Series` importé

En mode **groupe** (fichier OAR + fichier Energy Series combinés), les deux courbes sont superposées.

Fonctions disponibles :
- Filtre énergie par **Cluster**
- Filtre énergie par **Owner**
- **Reset** des filtres énergie
- **Ajuster à la figure** (checkbox) : recale automatiquement l'axe Y
- Survol du graphe : heure + puissance (W)
- Zoom/déplacement sur le graphe : recale la fenêtre temporelle du Gantt

Le **séparateur** entre le Gantt et le graphe énergie est **glissable verticalement** pour redimensionner les deux zones.

---

## 7) Vues d'agrégation du Gantt

### 7.1) Utilisation des vues

Le menu déroulant **View** (dans la barre d'outils du Gantt) permet de choisir la hiérarchie d'affichage des ressources.

Chaque vue définit :
- **Les niveaux hiérarchiques** : les ressources sont regroupées de gauche à droite (ex. site → cluster → hôte)
- **L'étiquette de chaque ligne** : dérivée d'un modèle configurable (ex. `{host|short}`)
- **Un filtre optionnel** : restreint les ressources affichées (ex. uniquement `production = YES`)

Exemples de vues prédéfinies :
- `Nodes` — vue standard des nœuds de calcul (site → cluster → host)
- `vlans` — vue des ressources réseau (site → type → vlan)

Les bandes colorées à gauche de la timeline représentent les niveaux hiérarchiques : une bande par niveau, du plus externe (gauche) au plus interne (droite). Survoler une bande affiche un tooltip récapitulatif du chemin (site, cluster, etc.).

---

### 7.2) Gestion des vues (Admin)

L'authentification **Admin** donne accès aux fonctionnalités de gestion des vues.

#### Créer une vue

Depuis le menu **View**, cliquer sur **+ Create view**.

Champs à remplir :
- **Name** : nom affiché dans le menu View
- **Leaf info preset** : jeu de champs affiché dans le tooltip au survol d'une ligne
- **Hierarchy levels** : niveaux de regroupement, du plus général au plus fin. Cliquer sur les champs disponibles pour les ajouter, utiliser ◀ / ▶ pour réordonner, 🗑 pour supprimer.
- **Leaf label template** : modèle d'étiquette pour les lignes feuilles. Exemples : `{host|short}`, `{type}/{vlan}`. Le modificateur `|short` tronque à la première partie avant le `.`.
- **Status bar fields** : champs affichés dans la barre de synthèse (vide = dernier niveau utilisé).
- **Sort by label** : trier les groupes par étiquette calculée (utile quand les clés sont des IDs opaques).
- **Resource filter** : filtre optionnel sur un champ de ressource. Choisir le champ, la valeur, et si la règle est une liste blanche (garder) ou liste noire (exclure).

Cliquer **Save view** pour enregistrer. La vue est immédiatement disponible dans le menu.

#### Modifier une vue

Dans le menu **View**, survoler une vue existante → cliquer ✏ à droite de son nom.

Le panneau **Edit view** s'ouvre avec les mêmes champs que la création. Modifier les champs souhaités puis cliquer **Apply**.

#### Supprimer une vue

Dans le menu **View**, survoler une vue → cliquer 🗑. Une fenêtre de confirmation s'affiche. Cliquer **Delete** pour confirmer.

> Note : la suppression est immédiate et irréversible. La configuration est sauvegardée dans `views.json`.

---

### 7.3) Presets d'informations (Leaf info presets)

Un **leaf info preset** définit les champs affichés dans le tooltip au survol d'une ligne de ressource (hôte, VLAN, disque, etc.).

#### Créer un preset

Dans le panneau **Create view** ou **Edit view**, cliquer **+** à côté du sélecteur de preset.

Remplir :
- **Preset name** : nom du preset
- **Fields** : cocher les champs à afficher (ex. `cluster`, `network_address`, `cputype`, `cpuset`, `memnode`…)

Un champ de recherche permet de filtrer la liste des champs disponibles.

Cliquer **Save preset**.

#### Modifier un preset

Dans le sélecteur de preset d'une vue, ouvrir la liste déroulante et cliquer ✏ sur le preset à modifier. Changer le nom et/ou les champs, puis **Apply**.

#### Supprimer un preset

Dans la liste déroulante des presets, cliquer 🗑 → confirmer dans la fenêtre de confirmation.

> Attention : supprimer un preset retire son affichage des tooltips pour toutes les vues qui l'utilisaient.

---

## 8) Presets de clusters (Admin)

Le bouton **Admin** est cliquable uniquement pour l'utilisateur `admin` et gris pour les autres.

Depuis le panneau **Admin configuration** :
- **New Preset** : créer un preset
- **Modify Preset** : modifier un preset existant
- Choix des clusters inclus (checkbox)
- **Save** : enregistre/écrase le preset
- **Delete** : supprime un preset

Les presets deviennent ensuite sélectionnables dans la fenêtre **Filtres**.

---

## 9) Dashboard

La vue **Dashboard** affiche :
- Un titre avec le nombre total de jobs filtrés
- Des **métriques** (boîtes colorées) : total jobs, jobs par état, plage de temps
- Ou un **graphique** par état de job (bouton bascule `Show charts` / `Show metrics`)
- Un **tableau de jobs** avec tri par colonne, pagination, et sélection des colonnes affichées

Cliquer sur une ligne du tableau ouvre la fenêtre de détails du job.

---

## 10) Import de fichiers

Via **Fichier → 📁 Import File**, une boîte de dialogue s'ouvre pour choisir un fichier JSON.

Après sélection, une fenêtre **Import File** apparaît :
- **Auto Detect** : détection automatique du type
- Ou sélection manuelle parmi les types disponibles

Types de fichiers supportés :
| Type | Contenu | Visualisation |
|------|---------|---------------|
| **OAR Simulation** | Jobs + ressources (format Grid5000/OAR) | Gantt + énergie estimée |
| **Energy Series** | Série temporelle de puissance mesurée | Graphe énergie |
| **Event** | Événements ponctuels sur des ressources nommées | Gantt (cercles) |

Cliquer **Import ▶** pour charger. Un onglet apparaît en haut du Gantt.

---

## 11) Groupement de fichiers

Il est possible de **combiner** un fichier OAR et un fichier Energy Series pour superposer la courbe mesurée sur l'estimation.

Pour grouper deux fichiers :
1. Importer le premier fichier (ex. OAR)
2. Sur son onglet, cliquer **`+`**
3. Choisir le second fichier (ex. Energy Series)
4. Les deux fichiers forment un **groupe** (onglet `groupN`)

Depuis l'onglet du groupe :
- `v` : affiche la liste des membres du groupe
- `+` : ajouter un fichier au groupe
- 🗑 sur un membre : supprimer ce fichier du groupe
- 🗑 sur le groupe : supprimer le groupe et tous ses fichiers

---

## 12) Résumé des fonctionnalités

- Authentification administrateur (gestion des vues et presets)
- Dashboard (métriques + graphique + tableau tri/pagination/colonnes)
- Gantt interactif (navigation, zoom, détails job)
- Vues d'agrégation configurables (hiérarchies, filtres, templates d'étiquettes)
- Presets d'informations pour les tooltips de ressources
- Filtres multi-critères + presets de clusters
- Import de fichiers JSON (OAR, Energy Series, Event)
- Groupement de sources (OAR + énergie superposés)
- Estimation et mesure énergétique synchronisées avec la timeline
- Thème clair/sombre, langue, taille de police
- Rafraîchissement auto et manuel (mode live uniquement)
