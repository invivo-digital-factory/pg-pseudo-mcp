# language: fr
@integration @querying
Fonctionnalité: Choix de la base
  Une connexion DBeaver pointe sur une seule base, alors que les bases applicatives vivent sur le même serveur : l'agent les liste puis interroge celle qu'il choisit.

  Contexte:
    Étant donné l'agent connecté à la base de test

  Scénario: L'agent liste les bases du serveur et repère celle de la connexion
    Quand l'agent liste les bases de la connexion
    Alors la liste contient la base "postgres"
    Et la liste contient la base de test
    Et la liste ne contient aucun modèle de base
    Et la base de la connexion est celle de test

  Scénario: L'agent interroge une seconde base avec l'argument database
    Quand l'agent interroge la base "postgres" avec la requête "SELECT current_database() = 'postgres'"
    Alors la requête réussit
    Et la première cellule de la réponse est vraie
    Quand l'agent exécute la requête "SELECT current_database() = 'postgres'"
    Alors la requête réussit
    Et la première cellule de la réponse est fausse

  Scénario: Le catalogue des bases reste fermé au SQL libre
    Quand l'agent exécute la requête "SELECT datname FROM pg_database"
    Alors la requête est refusée parce qu'elle n'est pas en lecture seule
