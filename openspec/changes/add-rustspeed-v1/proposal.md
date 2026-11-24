# Proposal

## Why

L'instance LibreSpeed (PHP) actuellement déployée dans Coolify sur le serveur « mars » doit être
remplacée par une alternative écrite en Rust, plus sobre en CPU/RAM, à protocole propre (non
compatible LibreSpeed), avec une UI moderne embarquée dans le binaire, un historique local des
résultats avec lien de partage, et des informations client (IP + FAI) obtenues sans aucun service
tiers. L'instance actuelle ne répond à aucun de ces besoins de souveraineté des données ni de
performance attendue.

## What Changes

- Création d'un nouveau service `rustspeed` (serveur HTTP en Rust, axum/tokio) exposant un
  protocole de mesure propre : download, upload, ping, jitter.
- Calcul des informations client (IP publique, FAI) sans appel à un service tiers (IP vue par le
  serveur, reverse DNS / PTR, base ASN hors ligne optionnelle), avec confiance en `X-Forwarded-For`
  uniquement depuis des proxies explicitement configurés (Traefik).
- Persistance des résultats de test dans une base SQLite locale, avec génération d'un identifiant
  de partage non devinable par résultat et une page de partage publique en lecture seule.
- UI front en Svelte, compilée et embarquée dans le binaire Rust via `rust-embed` (aucune
  dépendance réseau externe : pas de CDN, pas de police externe, pas d'analytics).
- Absence d'authentification : la seule barrière d'accès est le réseau Tailscale sur lequel le
  service est exposé (`speed.mars.lucasrosen.fr`).
- Configuration entièrement par variables d'environnement (ports, proxies de confiance, chemin de
  la base SQLite, rétention des résultats, etc.).
- Endpoint `/healthz` pour le healthcheck Docker/Coolify, logs structurés sur stdout.
- Image Docker multi-stage (build Svelte → build Rust → image minimale) et déploiement dans
  Coolify derrière Traefik (TLS Let's Encrypt, `X-Forwarded-For` assaini par Traefik).
- Licence MIT OR Apache-2.0, dépôt public.

## Capabilities

### New Capabilities
- `speed-measurement`: protocole de mesure propre du débit descendant, montant, de la latence
  (ping) et de la variation de latence (jitter), via HTTP/WebSocket.
- `client-info`: détermination de l'IP publique et du FAI du client sans service tiers, avec
  règles de confiance sur les en-têtes de proxy.
- `results-history`: persistance SQLite des résultats de test et génération de liens de partage
  non devinables, avec rétention configurable.
- `web-ui`: interface utilisateur Svelte compilée et embarquée dans le binaire, servie sans
  dépendance réseau externe.
- `deployment`: configuration par variables d'environnement, healthcheck, image Docker
  multi-stage, déploiement Coolify/Traefik.

### Modified Capabilities
(aucune — nouveau projet, aucune spec existante)

## Impact

- Nouveau binaire Rust unique embarquant le serveur HTTP et les assets front compilés.
- Nouvelle base de données SQLite (fichier monté sur un volume persistant Docker).
- Nouvelle image Docker et nouvelle stack Coolify, remplaçant l'instance LibreSpeed existante
  derrière Traefik sur `speed.mars.lucasrosen.fr`.
- Aucune compatibilité avec les clients ou serveurs LibreSpeed existants : migration = bascule
  complète du client et du serveur, sans reprise de l'historique LibreSpeed.
