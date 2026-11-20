# language: fr
@integration @connections
Fonctionnalité: Tunnel SSH vers la base
  Une base derrière un bastion n'est jointe que si la clé d'hôte du bastion est connue de l'utilisateur.

  Contexte:
    Étant donné une connexion DBeaver qui traverse le bastion SSH de test

  Scénario: Le bastion dont la clé d'hôte est connue ouvre la session
    Étant donné un fichier known_hosts qui connaît la clé du bastion
    Quand l'agent exécute une requête de lecture sur la connexion tunnelée
    Alors la session s'ouvre et répond

  Scénario: Un bastion inconnu est refusé avec son empreinte
    Étant donné un fichier known_hosts vide
    Quand l'agent exécute une requête de lecture sur la connexion tunnelée
    Alors la connexion est refusée
    Et le message de refus contient "not in known_hosts"
    Et le message de refus contient l'empreinte "SHA256:" de la clé du bastion

  Scénario: Une clé d'hôte qui a changé est refusée
    Étant donné un fichier known_hosts qui porte une autre clé pour le bastion
    Quand l'agent exécute une requête de lecture sur la connexion tunnelée
    Alors la connexion est refusée
    Et le message de refus contient "changed"
