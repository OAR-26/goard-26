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

## 7) Presets de clusters (Admin)

Le bouton **Admin** est cliquable uniquement pour l’utilisateur `admin` et gris pour les autres.

Depuis le panneau **Admin configuration** :
- **New Preset** : créer un preset
- **Modify Preset** : modifier un preset existant
- Choix des clusters inclus (checkbox)
- **Save** : enregistre/écrase le preset
- **Delete** : supprime un preset


Ensuite, ils deviennent utilisables dans la fenêtre **Filtres**.

---


## 8) Résumé des fonctionnalités

- Authentification
- Dashboard (métriques + graphique + tableau tri/pagination/colonnes)
- Gantt interactif (navigation, zoom, détails)
- Filtres multi-critères + presets de clusters
- Estimation énergétique synchronisée avec la timeline
- Thème, langue, taille de police
- Rafraîchissement auto et manuel
- Gestion admin des presets
