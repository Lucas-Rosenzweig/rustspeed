# Spec Delta: web-ui

## Purpose
Fournir une interface utilisateur moderne en Svelte, compilée en assets statiques et embarquée
dans le binaire Rust, sans aucune dépendance réseau vers un service tiers au runtime.

## ADDED Requirements

### Requirement: UI embarquée dans le binaire
Le binaire serveur SHALL servir les assets front (HTML, CSS, JS, polices) directement depuis sa
mémoire via les assets compilés et embarqués au build (`rust-embed`), sans dépendre d'un
répertoire de fichiers externe au runtime.

#### Scenario: Démarrage du binaire sans répertoire front externe
- **WHEN** le binaire rustspeed est démarré dans un conteneur ne contenant que l'exécutable (sans
  répertoire `dist/` ou `static/` sur disque)
- **THEN** l'UI est servie normalement, car les assets sont embarqués dans l'exécutable

### Requirement: Aucune dépendance réseau tierce au runtime
L'UI SHALL fonctionner sans émettre de requête réseau vers un domaine tiers (CDN, police externe,
script d'analytics, service de géolocalisation) : toutes les ressources (polices, icônes,
scripts, styles) SHALL être servies depuis l'origine du serveur rustspeed lui-même.

#### Scenario: Audit réseau de la page de test
- **WHEN** un utilisateur charge la page de test rustspeed et observe les requêtes réseau émises
  par le navigateur
- **THEN** aucune requête n'est adressée à un domaine différent de celui du serveur rustspeed

### Requirement: Déroulé de test dans l'UI
L'UI SHALL permettre de lancer un test complet (download, upload, ping, jitter) depuis une seule
page, d'afficher la progression de chaque mesure, puis d'afficher les informations client (IP,
FAI) et un lien de partage vers le résultat enregistré.

#### Scenario: Test complet depuis l'UI
- **WHEN** l'utilisateur clique sur le bouton de démarrage du test
- **THEN** l'UI exécute successivement les mesures de download, upload, ping/jitter, affiche les
  résultats numériques, les informations client et un lien de partage généré après l'enregistrement
  du résultat

### Requirement: Page de partage publique
L'UI SHALL fournir une page `/r/{share_token}` affichant en lecture seule les métriques d'un
résultat partagé, accessible sans authentification et ne permettant aucune action de modification
ou de suppression.

#### Scenario: Consultation d'un résultat partagé
- **WHEN** un visiteur ouvre un lien de partage valide
- **THEN** l'UI affiche les métriques du test correspondant sans proposer aucune action de
  modification ou de suppression
