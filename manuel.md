# Manuel utilisateur - Goard

Ce manuel décrit l’utilisation de l’application **Goard** et l’ensemble des fonctionnalités visibles dans l’interface.

## 1) Connexion

Au démarrage, pour accéder l'espace admine (créer/modifier les presets des clusters) il faut s'authentifier comme admin.

## Identifiants actuels
- **Utilisateur** : `admin`
- **Mot de passe** : `admin`

- Dans le menu **Fichier**, vous pouvez vous connecter/déconnecter selon l’état de session
- si non authentifier comme admin le boutton apparaît en gris.

> Note : l’authentification actuelle est une preuve de concept (identifiants codés en dur).

---

## 2) Barre de menu (haut)

## Fichier
- **Se connecter** (si non connecté)
- **Se déconnecter** (si connecté)
- Affichage de l’utilisateur connecté
- **Quitter** (ferme l’application)

## Options
Ouvre la fenêtre d’options pour :
- **Langue** : English / Français
- **Taille de police** : de 10 à 30
- **Enregistrer** : sauvegarde dans `options.json`

## Aide contextuelle (`?`)
- Affiche une aide différente selon la vue active (Dashboard ou Gantt)

---

## 3) Barre d’outils (sous le menu)

Fonctionnalités globales :
- **Mode** : bouton `Dashboard` / `Gantt`
- **Filtres** : bouton `Filtres`
- **Rafraîchissement automatique** : choix `30 s`, `1 min`, `5 min`
- **Rafraîchissement immédiat** : bouton `⟳`
- **Thème clair/sombre** : bouton `☀` / `🌙`

Comportement :
- Le bouton `⟳` est désactivé pendant un rafraîchissement en cours
- Un indicateur en bas (`Refreshing data...`) + spinner apparaît pendant l’actualisation

---

## 4) Filtres des jobs

La fenêtre **Filtres** permet de filtrer l’affichage par :
- **Propriétaire (Owner)**
- **État du job (State)**
- **Preset de clusters** (None ou preset nommé)

Boutons :
- **Appliquer** : applique les filtres et met à jour l’affichage
- **Réinitialiser** : remet les filtres par défaut

Les filtres impactent :
- le Dashboard (métriques + tableau)
- le Gantt
- le calcul énergétique (dans la vue Gantt)

---


## 5) Vue Gantt

La vue Gantt affiche les jobs et ressources sur une timeline interactive.

## Interactions principales
- **Glisser (clic gauche)** : déplacement horizontal
- **Zoom horizontal** : `Ctrl/Cmd + molette` ou glisser vertical clic droit
- **Zoom vertical** : `Alt/Option + molette`
- **Double clic gauche** : réinitialiser la vue
- **Clic gauche sur un job** : zoom sur le job
- **Clic droit sur un job** : ouvrir les détails

## Contrôles Gantt (barre outils)
- `Paramètres`
  - Agrégation (niveau 1 / niveau 2)
  - Couleur des jobs (aléatoire / par état)
- Navigation rapide : `◀ 1w`, `◀ 1d`, `1d ▶`, `1w ▶`
- `Centrer sur maintenant`

## Ligne de synthèse (en mode Gantt)
Affiche :
- Nombre de jobs filtrés
- Clusters affichés / total
- Hosts affichés / total
- État des données (`refreshing`, `loading`, `ready`)

## Détails job
Les fenêtres de détails restent ouvertes individuellement et peuvent être fermées séparément.

---

## 6) Énergie (vue Gantt)

Sous le Gantt, un graphe **Consommation globale (estimée)** est affiché.

Fonctions disponibles :
- Filtre énergie par **Cluster**
- Filtre énergie par **Owner**
- **Reset** des filtres énergie
- Survol du graphe : heure + puissance estimée (W)
- Zoom/déplacement sur le graphe : recale la fenêtre temporelle du Gantt

---

## 7) Vues d'agrégation du Gantt

### 7.1) Utilisation des vues

Le menu déroulant **View** (dans la barre d'outils du Gantt) permet de choisir la hiérarchie d'affichage des ressources.

Chaque vue définit :
- **Les niveaux hiérarchiques** : les ressources sont regroupées de gauche à droite (ex. site → cluster → hôte)
- **L'étiquette de chaque ligne** : dérivée d'un modèle configurable (ex. `{host|short}`)
- **Un filtre optionnel** : restreint les ressources affichées (ex. uniquement `type = default`)

Exemples de vues prédéfinies :
- `Compute: site → cluster → host` — vue standard des nœuds de calcul
- `Network: site → type → vlan` — vue des ressources réseau (Kavlan)

Les bandes colorées à gauche de la timeline représentent les niveaux hiérarchiques : une bande par niveau, du plus externe (gauche) au plus interne (droite). Survoler une bande affiche un tooltip récapitulatif du chemin (site, cluster, etc.).

---

### 7.2) Gestion des vues (Admin)

L'authentification **Admin** donne accès aux fonctionnalités de gestion des vues. Ces actions sont réservées à l'utilisateur `admin`.

#### Créer une vue

Depuis le menu **View**, cliquer sur **+ Create view**.

Champs à remplir :
- **Name** : nom affiché dans le menu View
- **Leaf info preset** : jeu de champs affiché dans le tooltip au survol d'une ligne
- **Hierarchy levels** : niveaux de regroupement, du plus général (ex. `site`) au plus fin (ex. `host`). Cliquer sur les champs disponibles pour les ajouter, utiliser ⬆ / ⬇ pour réordonner, 🗑 pour supprimer.
- **Leaf label template** : modèle d'étiquette pour les lignes feuilles. Exemples : `{host|short}`, `{type}/{vlan}`. Le modificateur `|short` tronque à la première partie avant le `.`.
- **Status bar fields** : champs affichés dans la barre de synthèse en haut du Gantt (vide = dernier niveau utilisé).
- **Sort by label** : trier les groupes par étiquette calculée plutôt que par clé brute (utile quand les clés sont des IDs opaques).
- **Resource filter** : filtre optionnel sur un champ de ressource. Cocher, sélectionner le champ et la valeur, puis choisir si la règle est une liste blanche (garder) ou liste noire (exclure).

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

Cliquer **Save preset**.

#### Modifier un preset

Dans le sélecteur de preset d'une vue, ouvrir la liste déroulante et cliquer ✏ sur le preset à modifier. Changer le nom et/ou les champs, puis **Apply**.

#### Supprimer un preset

Dans la liste déroulante des presets, cliquer 🗑 → confirmer dans la fenêtre de confirmation.

> Attention : supprimer un preset retire son affichage des tooltips pour toutes les vues qui l'utilisaient.

---

## 8) Presets de clusters (Admin)

Le bouton **Admin** est cliquable uniquement pour l’utilisateur `admin` et gris pour les autres.

Depuis le panneau **Admin configuration** :
- **New Preset** : créer un preset
- **Modify Preset** : modifier un preset existant
- Choix des clusters inclus (checkbox)
- **Save** : enregistre/écrase le preset
- **Delete** : supprime un preset


Ensuite, ils deviennent utilisables dans la fenêtre **Filtres**.

---


## 9) Résumé des fonctionnalités

- Authentification administrateur
- Dashboard (métriques + graphique + tableau tri/pagination/colonnes)
- Gantt interactif (navigation, zoom, détails job)
- Vues d'agrégation configurables (hiérarchies, filtres, templates d'étiquettes)
- Presets d'informations pour les tooltips de ressources
- Filtres multi-critères + presets de clusters
- Estimation énergétique synchronisée avec la timeline
- Thème clair/sombre, langue, taille de police
- Rafraîchissement auto et manuel
