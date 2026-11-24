# Spec Delta: deployment

## Purpose
Définir comment rustspeed est configuré, packagé en image Docker et déployé dans Coolify derrière
Traefik, sans authentification applicative et avec une rétention de données maîtrisée.

## ADDED Requirements

### Requirement: Configuration par variables d'environnement
Le serveur SHALL lire l'intégralité de sa configuration (adresse d'écoute, chemin de la base
SQLite, liste des proxies de confiance, rétention des résultats, chemin de la base ISP
optionnelle, niveau de log) depuis des variables d'environnement, avec des valeurs par défaut
sûres en leur absence, et SHALL refuser de démarrer si une valeur fournie est invalide.

#### Scenario: Démarrage avec configuration par défaut
- **WHEN** le serveur démarre sans aucune variable d'environnement `RUSTSPEED_*` définie
- **THEN** il démarre avec les valeurs par défaut documentées (écoute sur `0.0.0.0:8080`, aucun
  proxy de confiance, rétention de 90 jours)

#### Scenario: Valeur de configuration invalide
- **WHEN** `RUSTSPEED_RETENTION_DAYS` est défini à une valeur non numérique
- **THEN** le serveur refuse de démarrer et journalise une erreur explicite sur stdout

### Requirement: Healthcheck applicatif
Le serveur SHALL exposer un endpoint `GET /healthz` répondant 200 lorsque le serveur et l'accès à
la base SQLite sont fonctionnels, destiné au healthcheck Docker/Coolify.

#### Scenario: Healthcheck nominal
- **WHEN** le serveur fonctionne normalement et que la base SQLite est accessible
- **THEN** `GET /healthz` répond avec un statut 200

#### Scenario: Base SQLite inaccessible
- **WHEN** le fichier de base SQLite n'est pas accessible en écriture (ex. volume mal monté)
- **THEN** `GET /healthz` répond avec un statut d'erreur (5xx), signalant au healthcheck Docker que
  le conteneur est défaillant

### Requirement: Image Docker multi-stage unique
Le projet SHALL fournir un Dockerfile multi-stage construisant d'abord les assets Svelte, puis le
binaire Rust (intégrant ces assets via `rust-embed`), puis une image finale minimale ne contenant
que le binaire compilé et ses dépendances d'exécution strictement nécessaires.

#### Scenario: Build de l'image
- **WHEN** l'image Docker est construite avec `docker build`
- **THEN** l'image finale ne contient ni outillage Node.js, ni toolchain Cargo/Rust, uniquement le
  binaire exécutable et ses dépendances runtime minimales

### Requirement: Fonctionnement sans authentification derrière un périmètre réseau restreint
Le serveur SHALL fonctionner sans mécanisme d'authentification applicative, la barrière d'accès
étant assurée par le périmètre réseau (Tailscale) et la terminaison TLS/proxy (Traefik dans
Coolify) ; le serveur SHALL néanmoins ne faire confiance à l'en-tête `X-Forwarded-For` que lorsque
la requête provient d'un proxy listé dans `TRUSTED_PROXIES`.

#### Scenario: Déploiement derrière Traefik
- **WHEN** le service est déployé dans Coolify derrière Traefik avec `TRUSTED_PROXIES` réglé sur
  l'IP interne de Traefik et exposé sur `speed.mars.lucasrosen.fr` via le réseau Tailscale
- **THEN** toute requête atteignant le serveur est considérée comme provenant du réseau de
  confiance Tailscale/Traefik, sans qu'une authentification applicative supplémentaire ne soit
  requise

### Requirement: Rétention et volume persistant documentés
Le déploiement SHALL monter un volume Docker persistant sur le répertoire contenant
`RUSTSPEED_DATABASE_PATH`, afin que l'historique des résultats survive aux redéploiements, et la
politique de rétention effective SHALL être documentée dans la configuration de déploiement.

#### Scenario: Redéploiement de la stack Coolify
- **WHEN** la stack Coolify rustspeed est redéployée (nouvelle image, mise à jour)
- **THEN** le fichier SQLite monté en volume persiste et conserve l'historique des résultats
  antérieurs au redéploiement
