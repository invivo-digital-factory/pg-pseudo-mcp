# language: fr
@connections
Fonctionnalité: Liste des connexions
  L'outil list_connections montre à l'agent les connexions qu'il peut nommer, avec leur moteur, leur source, leur statut, leur base et leur type.
  Une connexion indisponible ou non prise en charge porte sa raison ; la liste ne montre jamais un hôte, un utilisateur, un secret ni le chemin du dossier personnel.

  Règle: Chaque connexion est listée avec son nom, son moteur, sa source, son statut, sa base et son type

    Scénario: Une connexion déclarée dans le fichier est listée comme disponible avec sa source
      Étant donné le fichier de connexions suivant :
        """
        [[connection]]
        name = "app-prod"
        host = "db.example.com"
        database = "shop"
        user = "alice"
        type = "prod"
        auth = "command"
        password_command = ["/bin/echo", "s3cret"]
        """
      Quand l'agent liste les connexions
      Alors la connexion "app-prod" est listée exactement avec les champs "name, engine, source, status, database, type"
      Et la connexion "app-prod" est listée comme disponible
      Et la connexion "app-prod" est listée avec la source "file"
      Et la connexion "app-prod" est listée avec le moteur "postgresql"
      Et la connexion "app-prod" est listée avec la base "shop"
      Et la connexion "app-prod" est listée avec le type "prod"
      Et la liste ne montre ni l'hôte "db.example.com", ni l'utilisateur "alice", ni le secret "s3cret"

  Règle: Seule une connexion indisponible ou non prise en charge porte une raison

    Scénario: Les trois statuts d'une liste
      Étant donné un espace de travail DBeaver avec une connexion "legacy" du fournisseur "mysql" au modèle d'authentification "native"
      Et le fichier de connexions suivant :
        """
        [[connection]]
        name = "app"
        host = "db.example.com"
        user = "alice"
        type = "dev"
        auth = "command"
        password_command = ["/bin/echo", "s3cret"]

        [[connection]]
        name = "broken"
        host = "db.example.com"
        user = "alice"
        type = "dev"
        auth = "pgpass"
        colour = "blue"
        """
      Quand l'agent liste les connexions
      Alors la connexion "app" est listée comme disponible
      Et la connexion "app" est listée sans raison
      Et la connexion "broken" est listée comme indisponible
      Et la raison de la connexion "broken" contient "unknown key `colour`"
      Et la connexion "legacy" est listée comme non prise en charge
      Et la raison de la connexion "legacy" contient "only PostgreSQL"

  Règle: Les connexions sont triées par nom, toutes sources confondues

    Scénario: Les connexions du fichier et de DBeaver sont listées dans l'ordre des noms
      Étant donné un espace de travail DBeaver où la connexion "canary-local" a son mot de passe enregistré
      Et le fichier de connexions suivant :
        """
        [[connection]]
        name = "zeta"
        host = "db.example.com"
        user = "alice"
        type = "dev"
        auth = "command"
        password_command = ["/bin/echo", "s3cret"]

        [[connection]]
        name = "alpha"
        host = "db.example.com"
        user = "alice"
        type = "dev"
        auth = "command"
        password_command = ["/bin/echo", "s3cret"]
        """
      Quand l'agent liste les connexions
      Alors les connexions sont listées dans l'ordre "alpha, canary-local, zeta"

  Règle: Le chemin du dossier personnel est remplacé par ~ dans les raisons et l'indice

    Scénario: Une clé SSH absente du dossier personnel est nommée depuis ~
      Étant donné un espace de travail DBeaver dont la connexion "tunnel-dev" passe par un tunnel SSH en "PUBLIC_KEY" avec la clé "{dossier personnel}/scarif-cle-absente-7731", l'utilisateur SSH "jump" et le mot de passe SSH ""
      Quand l'agent liste les connexions
      Alors la raison de la connexion "tunnel-dev" contient "SSH key file not found: ~/scarif-cle-absente-7731"
      Et la liste ne montre pas le chemin du dossier personnel

  Règle: Une connexion inconnue est refusée avec la liste des connexions disponibles

    Scénario: Une requête sur une connexion inconnue nomme les connexions disponibles, pas les indisponibles
      Étant donné le fichier de connexions suivant :
        """
        [[connection]]
        name = "app"
        host = "db.example.com"
        user = "alice"
        type = "dev"
        auth = "command"
        password_command = ["/bin/echo", "s3cret"]

        [[connection]]
        name = "broken"
        host = "db.example.com"
        user = "alice"
        type = "dev"
        auth = "pgpass"
        """
      Quand l'agent exécute une requête de lecture sur la connexion "absente"
      Alors la requête échoue avec une erreur qui contient "unknown connection 'absente'"
      Et l'erreur propose les connexions disponibles "app"
