# rustspeed

Test de débit réseau auto-hébergé, écrit en Rust. Alternative à
[LibreSpeed](https://github.com/librespeed/speedtest) avec un binaire unique,
une UI Svelte embarquée et aucune dépendance à un service tiers.

- **Mesures** : débit descendant, débit montant, ping (médiane) et gigue.
- **Infos client** : IP et FAI déterminés localement (reverse DNS + base IP→ASN
  hors ligne facultative). Aucun appel à ipinfo.io ou équivalent.
- **Historique** : résultats stockés dans SQLite, chacun avec un lien de partage
  non devinable (`/r/<token>`, 128 bits aléatoires) et une rétention configurable.
- **Souveraineté** : l'UI ne charge ni CDN, ni police externe, ni analytics.
- **Léger** : image Docker distroless d'environ 17 Mo, binaire statique (musl).

> Protocole propre : rustspeed n'est **pas** compatible avec les endpoints ni les
> clients LibreSpeed.

## Démarrage rapide

```sh
docker build -t rustspeed .
docker run -p 8080:8080 -v rustspeed-data:/data rustspeed
```

Puis ouvrir <http://localhost:8080>.

## Configuration

Toutes les options passent par des variables d'environnement. Une valeur
invalide empêche le démarrage.

| Variable | Défaut | Rôle |
|---|---|---|
| `RUSTSPEED_LISTEN_ADDR` | `0.0.0.0:8080` | Adresse d'écoute |
| `RUSTSPEED_DATABASE_PATH` | `/data/rustspeed.sqlite3` | Base SQLite |
| `RUSTSPEED_TRUSTED_PROXIES` | *(vide)* | IP/CIDR (séparées par des virgules) autorisées à fixer `X-Forwarded-For`. Vide = en-tête ignoré |
| `RUSTSPEED_RETENTION_DAYS` | `90` | Durée de conservation des résultats ; `0` = illimitée |
| `RUSTSPEED_ISP_DB_PATH` | *(aucun)* | Fichier IP→ASN hors ligne (voir ci-dessous) |
| `RUSTSPEED_LOG_LEVEL` | `info` | Filtre `tracing` (`debug`, `rustspeed=debug,info`…) |
| `RUSTSPEED_LOG_FORMAT` | `json` | `json` ou `text` |
| `RUSTSPEED_MAX_DOWNLOAD_BYTES` | `1073741824` | Taille max d'une requête de download |
| `RUSTSPEED_MAX_UPLOAD_BYTES` | `268435456` | Taille max d'une requête d'upload |

### Derrière un reverse proxy

Renseigner `RUSTSPEED_TRUSTED_PROXIES` avec le réseau du proxy (par ex. le réseau
Docker de Traefik). La chaîne `X-Forwarded-For` est lue de droite à gauche et le
premier saut non approuvé est retenu comme IP client : une valeur injectée par le
client à gauche de la chaîne est ignorée.

### Base FAI hors ligne (facultative)

Formats acceptés, une plage par ligne (`#` = commentaire) :

- CSV : `100.64.0.0/10,AS41231,Tailscale Inc`
- TSV [iptoasn.com](https://iptoasn.com/) (`ip2asn-combined.tsv`) :
  `début<TAB>fin<TAB>asn<TAB>pays<TAB>organisation`

Sans ce fichier, le FAI est déduit du nom reverse DNS (ex. `wanadoo.fr`).

## API (`/api/v1`)

| Méthode | Route | Description |
|---|---|---|
| `GET` | `/measure/download?bytes=N` | Flux de N octets aléatoires (incompressibles, `no-store`) |
| `POST` | `/measure/upload` | Consomme le corps, renvoie `{bytes, server_duration_ms}` |
| `GET` | `/measure/ws` | WebSocket d'écho pour le ping et la gigue |
| `GET` | `/client-info` | `{ip, hostname, isp, asn}` |
| `POST` | `/results` | Enregistre `{download_mbps, upload_mbps, ping_ms, jitter_ms}` et renvoie le `share_token` |
| `GET` | `/results?limit=N` | Derniers résultats |
| `GET` | `/results/{token}` | Un résultat (404 s'il est inconnu ou expiré) |
| `GET` | `/healthz` | 200 si la base répond |

Méthode de mesure côté navigateur : 6 flux parallèles pendant 10 s par sens ; les
2 premières secondes (montée en charge TCP) sont exclues du calcul. Le ping est la
médiane de 20 allers-retours WebSocket ; la gigue est la moyenne des écarts entre
RTT consécutifs (RFC 3550 simplifiée).

## Sécurité

Aucune authentification : l'accès doit être limité au niveau réseau (VPN,
Tailscale, liste blanche du reverse proxy). Toute personne qui peut joindre le
service voit l'historique des résultats et les IP qu'il contient.

## Développement

```sh
cd web && npm ci && npm run build && cd ..   # UI → web/dist (embarquée au build)
cargo test
cargo run                                    # http://localhost:8080
cd web && npm run dev                        # UI en hot-reload, API proxifiée vers :8080
```

La spécification (format [OpenSpec](https://github.com/Fission-AI/OpenSpec)) se
trouve dans [`openspec/`](openspec/) ; la synthèse du besoin dans
[`docs/interview-synthesis.md`](docs/interview-synthesis.md).

## Licence

Au choix : [MIT](LICENSE-MIT) ou [Apache-2.0](LICENSE-APACHE).
