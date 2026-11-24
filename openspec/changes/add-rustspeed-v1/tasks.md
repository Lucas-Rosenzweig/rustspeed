# Tasks

## 1. Socle serveur et configuration

- [x] 1.1 Initialiser le projet Rust (workspace, axum, tokio, tower, tracing) et la structure de
      dossiers (serveur, front Svelte)
- [x] 1.2 Implémenter le chargement de configuration depuis les variables d'environnement
      (`RUSTSPEED_LISTEN_ADDR`, `RUSTSPEED_DATABASE_PATH`, `RUSTSPEED_TRUSTED_PROXIES`,
      `RUSTSPEED_RETENTION_DAYS`, `RUSTSPEED_ISP_DB_PATH`, `RUSTSPEED_LOG_LEVEL`) avec valeurs par
      défaut et validation au démarrage
- [x] 1.3 Mettre en place les logs structurés sur stdout (tracing + tracing-subscriber, format
      JSON ou texte selon env)
- [x] 1.4 Implémenter l'endpoint `GET /healthz` (vérifie l'accès à la base SQLite)

## 2. Mesure réseau (speed-measurement)

- [x] 2.1 Implémenter `GET /api/v1/measure/download` : génération de flux binaire aléatoire
      incompressible, streaming chunké, en-têtes `Cache-Control: no-store`
- [x] 2.2 Implémenter `POST /api/v1/measure/upload` : lecture complète du flux entrant sans
      persistance, mesure de durée côté serveur, réponse JSON (octets reçus, durée)
- [x] 2.3 Implémenter l'endpoint WebSocket `GET /api/v1/measure/ws` pour ping/jitter : echo des
      trames horodatées, calcul serveur optionnel de contrôle
- [x] 2.4 Écrire les calculs client (download/upload Mbps, ping médian, jitter RFC 3550 simplifié)
      côté front (voir groupe 5)
- [x] 2.5 Tests d'intégration des trois endpoints de mesure (download/upload/ws) avec clients de
      test async

## 3. Informations client (client-info)

- [x] 3.1 Implémenter la détermination de l'IP client (IP de connexion TCP par défaut,
      `X-Forwarded-For` uniquement si la requête provient d'une IP dans `TRUSTED_PROXIES`)
- [x] 3.2 Implémenter la résolution DNS inverse (PTR) de l'IP client, avec timeout et repli sur
      `None` en cas d'échec
- [x] 3.3 Implémenter le chargement optionnel d'un fichier IP→ASN/organisation
      (`RUSTSPEED_ISP_DB_PATH`) et son utilisation pour enrichir le nom du FAI
- [x] 3.4 Exposer ces informations via un endpoint `GET /api/v1/client-info` consommé par l'UI
- [x] 3.5 Tests unitaires de la logique de confiance proxy (avec/sans proxy de confiance configuré,
      en-tête falsifié, absence d'en-tête)

## 4. Historique et partage (results-history)

- [x] 4.1 Définir le schéma SQLite (table `results`) et la migration embarquée (`sqlx migrate`)
- [x] 4.2 Implémenter `POST /api/v1/results` : enregistrement d'un résultat de test avec génération
      du `share_token` cryptographiquement aléatoire et calcul de `expires_at`
- [x] 4.3 Implémenter `GET /api/v1/results/:share_token` : lecture d'un résultat en lecture seule,
      404 si inexistant/expiré
- [x] 4.4 Implémenter la tâche périodique de purge selon `RUSTSPEED_RETENTION_DAYS`
- [x] 4.5 Tests d'intégration : création, lecture par token, non-devinabilité (format/entropie du
      token), purge après expiration

## 5. UI Web (web-ui)

- [x] 5.1 Construire le projet Svelte (page de test, page de résultat/partage), sans dépendance
      réseau externe (polices et icônes servies localement)
- [x] 5.2 Implémenter le déroulé client du test (download, upload, ping/jitter via WebSocket) et
      l'affichage des résultats
- [x] 5.3 Implémenter la page de partage publique `/r/:share_token` (lecture seule)
- [x] 5.4 Intégrer `rust-embed` côté serveur Rust pour servir les assets Svelte compilés depuis le
      binaire
- [x] 5.5 Vérifier qu'aucune requête réseau sortante vers un domaine tiers n'est émise par l'UI
      (audit manuel des requêtes réseau en build de production)

## 6. Déploiement (deployment)

- [x] 6.1 Écrire le Dockerfile multi-stage (frontend Node → builder Rust → image finale minimale)
- [x] 6.2 Déclarer `HEALTHCHECK` Docker sur `/healthz` et exposer le port configuré
- [x] 6.3 Documenter le volume Docker persistant pour `RUSTSPEED_DATABASE_PATH`
- [x] 6.4 Configurer le déploiement Coolify derrière Traefik pour `speed.mars.lucasrosen.fr`
      (TLS Let's Encrypt, `TRUSTED_PROXIES` réglé sur l'IP interne de Traefik)
- [x] 6.5 Mettre en place la CI GitHub Actions (fmt, clippy, tests Rust, build front, build image
      Docker)
- [x] 6.6 Ajouter les fichiers de licence (MIT OR Apache-2.0) et le README du dépôt public

## 7. Validation finale

- [x] 7.1 Exécuter `openspec validate --strict` sur la proposition et corriger tout écart
- [x] 7.2 Vérifier manuellement un cycle complet (test depuis l'UI → résultat persisté → lien de
      partage consulté sans authentification)
