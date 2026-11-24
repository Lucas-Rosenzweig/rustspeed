# Spec Delta: client-info

## Purpose
Déterminer l'adresse IP publique et le fournisseur d'accès à Internet (FAI) du client, sans
appeler aucun service tiers, en respectant une politique stricte de confiance des en-têtes de
proxy.

## ADDED Requirements

### Requirement: Détermination de l'IP client sans service tiers
Le serveur SHALL déterminer l'IP du client à partir de la connexion TCP entrante, sauf si la
requête provient d'une adresse listée dans la configuration `TRUSTED_PROXIES`, auquel cas le
serveur MAY utiliser la valeur pertinente de l'en-tête `X-Forwarded-For`. Aucun appel réseau vers
un service tiers ne SHALL être effectué pour cette détermination.

#### Scenario: Requête directe sans proxy de confiance configuré
- **WHEN** une requête arrive sans `TRUSTED_PROXIES` configuré, avec un en-tête
  `X-Forwarded-For` arbitraire fourni par le client
- **THEN** le serveur ignore l'en-tête `X-Forwarded-For` et utilise l'IP de la connexion TCP comme
  IP client

#### Scenario: Requête via Traefik configuré comme proxy de confiance
- **WHEN** `TRUSTED_PROXIES` contient l'IP interne de Traefik et une requête arrive depuis cette
  IP avec un en-tête `X-Forwarded-For` valide
- **THEN** le serveur utilise la valeur appropriée de `X-Forwarded-For` comme IP client

### Requirement: Résolution du FAI sans service tiers
Le serveur SHALL tenter de déterminer le nom du fournisseur d'accès du client par résolution DNS
inverse (requête PTR) sur l'IP client, sans appeler de service de géolocalisation ou d'API FAI
tierce. Si une base de correspondance IP→organisation hors ligne est configurée
(`RUSTSPEED_ISP_DB_PATH`), le serveur MAY l'utiliser pour enrichir le résultat.

#### Scenario: Résolution rDNS réussie
- **WHEN** l'IP client dispose d'un enregistrement PTR valide
- **THEN** le serveur renvoie ce nom d'hôte comme information FAI, sans effectuer d'appel réseau
  vers un service tiers

#### Scenario: Résolution rDNS échouée ou absente
- **WHEN** la résolution PTR échoue, time-out, ou qu'aucun enregistrement n'existe
- **THEN** le serveur renvoie une valeur FAI nulle/absente plutôt que d'échouer la requête, et
  n'effectue aucun appel de repli vers un service tiers

### Requirement: Exposition des informations client à l'UI
Le serveur SHALL exposer un endpoint `GET /api/v1/client-info` renvoyant l'IP et le FAI déterminés
pour la requête courante, consommé par l'UI pour affichage avant/après le test.

#### Scenario: L'UI récupère les informations client
- **WHEN** l'UI appelle `GET /api/v1/client-info`
- **THEN** le serveur répond en JSON avec l'IP déterminée selon la politique de confiance proxy et
  le FAI (ou null) déterminé par résolution rDNS/base hors ligne
