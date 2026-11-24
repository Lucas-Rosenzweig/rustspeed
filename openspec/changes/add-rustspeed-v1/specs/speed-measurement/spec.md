# Spec Delta: speed-measurement

## Purpose
Mesurer le débit descendant, le débit montant, la latence (ping) et la variation de latence
(jitter) entre le client et le serveur rustspeed, via un protocole propre non compatible
LibreSpeed.

## ADDED Requirements

### Requirement: Mesure du débit descendant
Le serveur SHALL exposer un endpoint `GET /api/v1/measure/download` qui répond par un flux
streamé de données binaires générées côté serveur, de taille demandée par le client, composées de
contenu aléatoire incompressible, sans en-tête de cache HTTP permettant une réponse mise en cache.

#### Scenario: Le client démarre un test de téléchargement
- **WHEN** le client envoie `GET /api/v1/measure/download?bytes=104857600`
- **THEN** le serveur répond en flux chunké avec exactement 104857600 octets de données
  aléatoires incompressibles et l'en-tête `Cache-Control: no-store`

### Requirement: Mesure du débit montant
Le serveur SHALL exposer un endpoint `POST /api/v1/measure/upload` qui consomme intégralement le
corps de la requête sans le persister ni le journaliser, mesure la durée de réception côté
serveur, et répond avec le nombre d'octets reçus et la durée mesurée.

#### Scenario: Le client envoie des données pour un test d'upload
- **WHEN** le client envoie `POST /api/v1/measure/upload` avec un corps binaire de 50 Mo de
  contenu aléatoire
- **THEN** le serveur lit la totalité du flux, ne le stocke sur aucun support persistant, et
  répond avec un JSON contenant le nombre d'octets reçus et la durée mesurée en millisecondes

### Requirement: Mesure du ping et du jitter via WebSocket
Le serveur SHALL exposer un endpoint WebSocket `GET /api/v1/measure/ws` qui renvoie en écho
(echo), sans délai de traitement artificiel, chaque trame reçue du client, afin de permettre un
calcul de round-trip time (RTT) par trame côté client.

#### Scenario: Le client mesure le ping et le jitter
- **WHEN** le client ouvre une connexion WebSocket sur `/api/v1/measure/ws` et envoie une série de
  trames horodatées à intervalle régulier
- **THEN** le serveur renvoie chaque trame reçue en écho immédiatement, permettant au client de
  calculer un ping médian et un jitter (moyenne des écarts absolus entre RTT consécutifs) à partir
  des RTT observés

### Requirement: Absence de compatibilité avec le protocole LibreSpeed
Le serveur SHALL n'exposer aucun endpoint, format de requête/réponse ou sémantique reprenant le
protocole LibreSpeed ; le protocole de mesure est propre à rustspeed et vit sous le préfixe
`/api/v1/measure/`.

#### Scenario: Un client LibreSpeed existant tente de se connecter
- **WHEN** un client LibreSpeed non modifié appelle les endpoints historiques de LibreSpeed
  (ex. `/garbage.php`, `/empty.php`) sur le serveur rustspeed
- **THEN** le serveur répond par une erreur 404 (route inexistante), confirmant l'absence de
  compatibilité protocolaire
