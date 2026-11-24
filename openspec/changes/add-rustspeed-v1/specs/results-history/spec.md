# Spec Delta: results-history

## Purpose
Persister chaque résultat de test dans une base SQLite locale et permettre son partage via un lien
public non devinable, avec une rétention configurable.

## ADDED Requirements

### Requirement: Persistance des résultats dans SQLite
Le serveur SHALL enregistrer chaque résultat de test complet (download, upload, ping, jitter, IP
client, FAI, horodatage) dans une base de données SQLite locale dont le chemin est configurable
via `RUSTSPEED_DATABASE_PATH`.

#### Scenario: Un test complet est enregistré
- **WHEN** un client termine un test de mesure complet (download + upload + ping + jitter) et
  appelle `POST /api/v1/results` avec les métriques mesurées
- **THEN** le serveur insère une nouvelle ligne dans la base SQLite avec un identifiant interne,
  les métriques, l'IP/FAI déterminés, et l'horodatage de création

### Requirement: Génération d'un identifiant de partage non devinable
Le serveur SHALL générer, pour chaque résultat enregistré, un `share_token` unique issu d'un
générateur aléatoire cryptographiquement sûr, avec une entropie au moins équivalente à un UUID v4
(122 bits), ne dérivant d'aucune donnée prévisible (horodatage, IP, compteur séquentiel).

#### Scenario: Deux résultats consécutifs ont des tokens indépendants
- **WHEN** deux tests sont enregistrés à quelques millisecondes d'intervalle
- **THEN** leurs `share_token` respectifs ne présentent aucune corrélation exploitable (ni préfixe
  commun dérivé du temps, ni suite arithmétique)

### Requirement: Consultation publique d'un résultat par lien de partage
Le serveur SHALL exposer un endpoint public `GET /api/v1/results/{share_token}` (et une page UI
correspondante `/r/{share_token}`) permettant de consulter un résultat en lecture seule, sans
authentification, et SHALL répondre 404 si le token est inconnu ou si le résultat a expiré.

#### Scenario: Consultation d'un résultat existant
- **WHEN** un visiteur ouvre `/r/{share_token}` avec un token valide et non expiré
- **THEN** le serveur affiche les métriques du résultat correspondant, sans qu'aucune
  authentification ne soit requise

#### Scenario: Consultation d'un token invalide ou expiré
- **WHEN** un visiteur appelle `GET /api/v1/results/{share_token}` avec un token inexistant ou
  expiré
- **THEN** le serveur répond avec un statut 404

### Requirement: Rétention configurable des résultats
Le serveur SHALL purger automatiquement les résultats dont l'âge dépasse
`RUSTSPEED_RETENTION_DAYS` jours, sauf si cette variable vaut `0`, auquel cas aucune purge
automatique ne SHALL avoir lieu (conservation indéfinie).

#### Scenario: Purge après la période de rétention
- **WHEN** `RUSTSPEED_RETENTION_DAYS` vaut 90 et qu'un résultat a été créé il y a plus de 90 jours
- **THEN** une tâche de purge périodique supprime ce résultat de la base SQLite

#### Scenario: Rétention désactivée
- **WHEN** `RUSTSPEED_RETENTION_DAYS` vaut 0
- **THEN** aucun résultat n'est supprimé automatiquement, quelle que soit son ancienneté
