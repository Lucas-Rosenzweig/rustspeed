# rustspeed — synthèse de l'interview de besoin

Projet : réécriture en Rust d'une alternative à LibreSpeed (test de débit self-hosted),
qui remplace l'instance LibreSpeed déployée dans Coolify sur le serveur « mars ».

## Décisions de l'utilisateur

| Sujet | Décision |
|---|---|
| Protocole | **Protocole propre**, pas de compatibilité avec les endpoints LibreSpeed ni ses clients |
| Mesures | Download + Upload + Ping + Jitter |
| Infos client | IP + FAI du client, **sans aucun service tiers** (pas d'ipinfo.io, etc.) |
| Multi-serveurs | Non (un seul serveur) |
| Historique | **SQLite local** + lien de partage par résultat |
| UI | Moderne, **framework front (Svelte) compilé puis embarqué dans le binaire** |
| Accès | Sans authentification ; seule barrière = réseau Tailscale |
| Licence | MIT OR Apache-2.0 |
| Nom | `rustspeed`, repo public `github.com/Lucas-Rosenzweig/rustspeed` |
| Domaine | `speed.mars.lucasrosen.fr` (wildcard `*.mars` → IP Tailscale, TLS Let's Encrypt via Traefik/Coolify) |

## Contexte et contraintes

- Souveraineté des données : aucune requête vers un tiers (pas de CDN, polices, analytics, géoloc externe).
- Déploiement : image Docker unique (multi-stage : build Svelte → build Rust → image minimale), dans Coolify,
  derrière Traefik (qui fournit `X-Forwarded-For` assaini ; ne faire confiance à cet en-tête que depuis un proxy de confiance configurable).
- Volume persistant pour la base SQLite.

## Hypothèses par défaut (non demandées, à valider dans la spec)

- Plus sobre en CPU/RAM que LibreSpeed PHP ; débit limité par le réseau et non par le serveur (≥ 10 Gbit/s en local visé).
- Logs structurés sur stdout ; endpoint `/healthz` pour le healthcheck Docker/Coolify.
- Configuration par variables d'environnement, pas de panneau d'administration en v1.
- FAI sans service tiers : reverse DNS (PTR) + base ASN hors ligne optionnelle (ex. fichier IP→ASN monté), sinon IP + rDNS seulement.
- Navigateurs modernes uniquement ; UI en français, anglais possible.
- CI GitHub Actions : fmt, clippy, tests, build front, build image.
- Pas de purge automatique en v1 ? → à trancher dans la spec (proposer une rétention configurable).
