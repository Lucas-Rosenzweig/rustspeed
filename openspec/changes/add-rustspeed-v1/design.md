# Design

## Context

LibreSpeed (PHP) est actuellement déployé dans Coolify, derrière Traefik, sur le serveur « mars »,
accessible uniquement via le réseau Tailscale. Il doit être remplacé par un service équivalent
réécrit en Rust, à protocole propre (aucune compatibilité avec les endpoints ou clients
LibreSpeed), plus sobre en ressources, avec une UI moderne embarquée et un historique local des
résultats partageable par lien. Contraintes fortes : souveraineté des données (aucun appel
réseau vers un tiers, y compris pour la géolocalisation/FAI), serveur unique (pas de
multi-serveurs en v1), pas d'authentification (la barrière d'accès est le réseau Tailscale),
licence MIT OR Apache-2.0, déploiement par image Docker unique dans Coolify derrière Traefik, avec
volume persistant pour SQLite.

## Goals / Non-Goals

**Goals:**
- Fournir des mesures fiables de download, upload, ping et jitter, limitées par le réseau et non
  par le serveur (objectif indicatif ≥ 10 Gbit/s en réseau local).
- Déterminer IP + FAI du client sans aucun service tiers.
- Persister les résultats dans SQLite et permettre un partage par lien non devinable.
- Servir une UI Svelte moderne, entièrement embarquée dans le binaire (zéro dépendance réseau
  externe au runtime : pas de CDN, pas de police Google Fonts, pas d'analytics).
- Fonctionner sans authentification, dans une configuration entièrement pilotée par variables
  d'environnement, avec healthcheck `/healthz` et logs structurés sur stdout.
- Être packagé en une image Docker unique (multi-stage) déployable telle quelle dans Coolify
  derrière Traefik.

**Non-Goals:**
- Compatibilité protocolaire ou client avec LibreSpeed.
- Support multi-serveurs / sélection de serveur par le client.
- Authentification utilisateur, comptes, panneau d'administration web (v1).
- Géolocalisation ou identification FAI via un service tiers (ipinfo.io, MaxMind en ligne, etc.).
- Purge automatique sans configuration explicite (la rétention doit être un choix exprimé,
  jamais un comportement caché).

## Decisions

### Stack serveur : axum + tokio
Axum (sur tokio) est retenu pour le serveur HTTP/WebSocket : écosystème mature, bonnes
performances asynchrones, intégration naturelle avec `tower` pour les middlewares (journalisation,
limites de taille de requête), et bon support du streaming de réponse nécessaire au test de
téléchargement. Alternative écartée : actix-web (plus de boilerplate pour le cas d'usage, pas
d'avantage net ici).

### Protocole de mesure propre (HTTP + WebSocket)
Le protocole n'a aucune compatibilité avec LibreSpeed (endpoints, formats de payload et sémantique
tous différents) :

- **Download** : `GET /api/v1/measure/download?bytes=<n>` — le serveur répond en flux (streaming
  chunké) un corps binaire de taille `bytes` composé de données générées aléatoirement côté
  serveur donc **incompressibles** (impossible pour un proxy/CDN de servir une version compressée
  qui fausserait la mesure). Pas de cache HTTP (`Cache-Control: no-store`). Le client chronomètre
  la réception complète côté navigateur et calcule le débit ; plusieurs requêtes parallèles
  (configurable côté client) permettent de saturer la liaison.
- **Upload** : `POST /api/v1/measure/upload` — le client envoie un corps binaire de taille
  négociée (générée aléatoirement côté client, donc également incompressible) ; le serveur lit et
  consomme le flux entièrement sans le stocker ni le journaliser, puis répond avec le nombre
  d'octets reçus et la durée mesurée côté serveur (utilisée en recoupement du chronométrage
  client).
- **Ping / Jitter** : connexion WebSocket `GET /api/v1/measure/ws` passée en mode ping : le client
  envoie des trames minimales horodatées à intervalle régulier, le serveur renvoie immédiatement
  chaque trame (echo) ; le client calcule le round-trip time (RTT) par trame, le ping retenu est la
  médiane (plus robuste aux valeurs aberrantes qu'une moyenne), et le jitter est la moyenne des
  écarts absolus entre RTT consécutifs (méthode RFC 3550 simplifiée). Le WebSocket est préféré à
  des requêtes HTTP successives pour éviter le coût de poignée de main (handshake) TCP/TLS répété
  qui polluerait la mesure de latence.
- Toutes les routes de mesure sont sous le préfixe `/api/v1/measure/` pour permettre une évolution
  de version ultérieure sans rupture silencieuse.

### Informations client sans service tiers
- IP du client : adresse de la connexion TCP entrante, ou valeur du dernier maillon de confiance
  dans `X-Forwarded-For` **uniquement** si la requête provient d'une IP listée dans
  `TRUSTED_PROXIES` (ex. l'IP interne de Traefik). Sans configuration de proxy de confiance ou si
  la requête ne vient pas d'un proxy listé, `X-Forwarded-For` est ignoré et l'IP de connexion est
  utilisée telle quelle.
- FAI : résolution DNS inverse (requête PTR) sur l'IP client, effectuée directement par le
  processus Rust (résolveur DNS configuré par le système / variable d'environnement), sans appel à
  une API tierce. Un fichier optionnel de correspondance IP→ASN/organisation (format CSV/MMDB
  monté en volume, chargé au démarrage) peut enrichir le nom du FAI si fourni ; en son absence,
  seul le nom rDNS est affiché. Aucun appel réseau sortant vers un service de géolocalisation.

### Schéma SQLite
Bibliothèque : `sqlx` (driver SQLite, macros de requêtes vérifiées à la compilation, pool async
compatible tokio). Migrations embarquées dans le binaire (`sqlx migrate` au démarrage).

Table `results` :
| Colonne | Type | Notes |
|---|---|---|
| `id` | TEXT PRIMARY KEY | UUID v4 interne |
| `share_token` | TEXT UNIQUE NOT NULL | identifiant de partage, voir ci-dessous |
| `created_at` | TEXT NOT NULL | horodatage ISO 8601 UTC |
| `client_ip` | TEXT NOT NULL | IP déterminée selon la règle de confiance proxy |
| `client_isp` | TEXT NULL | nom rDNS / FAI, peut être NULL si non résolu |
| `download_mbps` | REAL NOT NULL | |
| `upload_mbps` | REAL NOT NULL | |
| `ping_ms` | REAL NOT NULL | médiane |
| `jitter_ms` | REAL NOT NULL | |
| `user_agent` | TEXT NULL | pour contexte d'affichage uniquement |
| `expires_at` | TEXT NULL | calculé depuis la rétention configurée, NULL = jamais |

Un index unique sur `share_token` et un index sur `created_at` (pour les purges et tris)
complètent le schéma.

### Identifiant de partage non devinable
`share_token` est généré via un générateur aléatoire cryptographiquement sûr (`rand::rngs::OsRng`
ou équivalent), encodé en base62 ou URL-safe base64, sur au moins 122 bits d'entropie (équivalent
UUID v4), afin qu'il soit statistiquement impossible à deviner ou à énumérer. Il ne dérive d'aucune
donnée prévisible (ni timestamp, ni IP, ni compteur). L'URL de partage est
`https://speed.mars.lucasrosen.fr/r/<share_token>` et ne nécessite aucune authentification pour la
consultation (lecture seule, aucune action de mutation possible depuis cette page).

### Front Svelte embarqué via rust-embed
Le front Svelte est compilé en assets statiques (HTML/CSS/JS/polices locales uniquement — aucune
police ni script chargé depuis un CDN externe) lors du build Docker (étape `node`), puis embarqué
dans le binaire Rust final via `rust-embed` (étape `rust`). Le binaire final ne dépend d'aucun
fichier externe pour servir l'UI : un seul artefact exécutable + la base SQLite externe sur
volume. Étant servi par le même process et la même origine que l'API de mesure, aucune
configuration CORS n'est nécessaire pour l'usage normal.

### Configuration par variables d'environnement
Toutes les options sont définies par variables d'environnement, avec valeurs par défaut sûres :
- `RUSTSPEED_LISTEN_ADDR` (défaut `0.0.0.0:8080`)
- `RUSTSPEED_DATABASE_PATH` (défaut `/data/rustspeed.sqlite3`)
- `RUSTSPEED_TRUSTED_PROXIES` (liste d'IP/CIDR séparées par des virgules, défaut vide = aucune
  confiance accordée à `X-Forwarded-For`)
- `RUSTSPEED_RETENTION_DAYS` (défaut `90` ; `0` = conservation indéfinie)
- `RUSTSPEED_ISP_DB_PATH` (optionnel, chemin du fichier IP→ASN/organisation hors ligne)
- `RUSTSPEED_LOG_LEVEL` (défaut `info`)
Aucune de ces variables n'est secrète ; aucun secret n'est requis en v1 (pas d'authentification).

### Dockerfile multi-stage et healthcheck
1. Étape `frontend` (image `node:lts-slim`) : installe les dépendances, build Svelte en mode
   production, produit les assets statiques.
2. Étape `builder` (image `rust:slim`, cible stable) : copie les assets de l'étape `frontend` à
   l'emplacement attendu par `rust-embed`, compile le binaire en mode release.
3. Étape finale (image minimale, ex. `debian:bookworm-slim` ou `gcr.io/distroless`) : copie
   uniquement le binaire compilé ; expose le port HTTP ; déclare `HEALTHCHECK` sur
   `GET /healthz` (code 200 attendu). Aucune dépendance Node/Cargo ne subsiste dans l'image finale.
   Volume Docker dédié monté sur le répertoire contenant `RUSTSPEED_DATABASE_PATH` pour persister
   l'historique entre redéploiements Coolify.

### Rétention configurable
Une tâche périodique interne (tokio, intervalle configurable, ex. toutes les heures) supprime les
lignes de `results` dont `expires_at` est dans le passé, lorsque `RUSTSPEED_RETENTION_DAYS > 0`.
Avec `RUSTSPEED_RETENTION_DAYS = 0`, aucune purge n'a lieu (conservation indéfinie), choix explicite
plutôt qu'un comportement implicite.

## Risks / Trade-offs

- **Mesure de débit depuis un navigateur** : le débit mesuré dépend aussi des capacités du
  navigateur/réseau client, pas seulement du serveur ; documenté comme limite connue, pas un bug.
- **FAI sans service tiers** : la résolution rDNS seule est souvent moins précise/complète qu'un
  service commercial de géo-IP ; accepté comme compromis de souveraineté des données, avec un
  mécanisme optionnel d'enrichissement hors ligne (fichier IP→ASN monté) pour qui veut plus de
  précision sans appel réseau externe.
- **Pas d'authentification** : la sécurité repose entièrement sur le périmètre réseau (Tailscale +
  Traefik). Toute exposition accidentelle hors Tailscale rendrait l'historique et les liens de
  partage consultables par quiconque possède l'URL ; à documenter explicitement dans les specs de
  déploiement.
- **rust-embed et taille du binaire** : les assets front embarqués augmentent la taille du binaire
  final, jugé acceptable face au bénéfice d'un artefact de déploiement unique.
