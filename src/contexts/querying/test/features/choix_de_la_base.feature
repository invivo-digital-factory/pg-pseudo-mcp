# language: fr
@integration @querying
Fonctionnalité: Choix de la base
  Une connexion pointe sur une seule base, alors que les bases applicatives vivent sur le même serveur : l'agent liste les bases du serveur,
  repère celle de la connexion, puis interroge celle qu'il choisit avec l'argument database. Les noms de bases ne sont pas des données personnelles.

  Règle: PostgreSQL : list_databases rend les bases connectables du serveur et celle de la connexion

    Contexte:
      Étant donné l'agent connecté à la base de test

    Scénario: L'agent liste les bases du serveur et repère celle de la connexion
      Quand l'agent liste les bases de la connexion
      Alors la liste contient la base "postgres"
      Et la liste contient la base de test
      Et la liste ne contient aucun modèle de base
      Et la base de la connexion est celle de test

    Scénario: Le catalogue des bases reste fermé au SQL libre
      Quand l'agent exécute la requête "SELECT datname FROM pg_database"
      Alors la requête est refusée parce qu'elle n'est pas en lecture seule

  Règle: PostgreSQL : l'argument database dirige la requête vers une autre base du même serveur

    Contexte:
      Étant donné l'agent connecté à la base de test

    Scénario: L'agent interroge une seconde base avec l'argument database
      Quand l'agent interroge la base "postgres" avec la requête "SELECT current_database() = 'postgres'"
      Alors la requête réussit
      Et la première cellule de la réponse est vraie
      Quand l'agent exécute la requête "SELECT current_database() = 'postgres'"
      Alors la requête réussit
      Et la première cellule de la réponse est fausse

    Scénario: Une base inconnue rend l'erreur du serveur
      Quand l'agent interroge la base "base_absente" avec la requête "SELECT 1"
      Alors l'outil est refusé avec le code "connection_failed"
      Et la réponse contient "base_absente"

  Règle: MongoDB : list_databases rend les bases autorisées sans les bases du serveur

    Scénario: Les bases admin, local et config ne sont jamais listées
      Étant donné la connexion MongoDB de test à travers le bastion
      Quand l'agent liste les bases de la connexion MongoDB
      Alors la liste contient la base "shop"
      Et la base de la connexion est "shop"
      Et la liste ne contient ni "admin", ni "local", ni "config"

  Règle: MongoDB : sans base par défaut, chaque lecture nomme sa base

    Scénario: Une connexion sans base demande l'argument database
      Étant donné la connexion MongoDB de test sans base par défaut
      Quand l'agent cherche dans "customers" tous les documents
      Alors l'outil est refusé avec le code "mongo_database_required"
      Quand l'agent cherche dans la base "shop" de la collection "customers" le document d'identifiant "65a000000000000000000001"
      Alors la réponse compte 1 documents
