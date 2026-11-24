# language: fr
@connections
Fonctionnalité: Fichier de connexions
  L'agent lit les connexions du fichier propre à scarif, qui ne contient aucun secret, et garde DBeaver comme source de repli.

  Scénario: Une connexion déclarée dans le fichier est listée comme disponible avec sa source
    Étant donné le fichier de connexions suivant :
      """
      [[connection]]
      name = "app-prod"
      host = "db.internal.example"
      user = "reader"
      type = "prod"
      auth = "command"
      password_command = ["/bin/echo", "reader-secret"]
      """
    Quand l'agent liste les connexions
    Alors la connexion "app-prod" est listée comme disponible
    Et la connexion "app-prod" est listée avec la source "file"
    Et la liste ne montre ni hôte, ni utilisateur, ni mot de passe

  @ae1
  Scénario: Une connexion du même nom dans le fichier et dans DBeaver n'est listée qu'une fois
    Étant donné un espace de travail DBeaver où la connexion "canary-local" a son mot de passe enregistré
    Et le fichier de connexions suivant :
      """
      [[connection]]
      name = "canary-local"
      host = "db.internal.example"
      user = "reader"
      type = "dev"
      auth = "command"
      password_command = ["/bin/echo", "reader-secret"]
      """
    Quand l'agent liste les connexions
    Alors la connexion "canary-local" n'est listée qu'une seule fois
    Et la connexion "canary-local" est listée avec la source "file"

  @ae2
  Scénario: Un fichier avec une erreur de syntaxe est signalé avec sa ligne et rend les connexions DBeaver indisponibles
    Étant donné un espace de travail DBeaver avec les connexions "dbeaver-a" et "dbeaver-b" dont les mots de passe sont enregistrés
    Et le fichier de connexions suivant :
      """
      # connexions de l'équipe
      #
      #
      #
      #
      #
      #
      #
      #
      #
      [[connection]]
      name = "a" "b"
      """
    Quand l'agent liste les connexions
    Alors l'indice cite la ligne 12
    Et la connexion "dbeaver-a" est listée comme indisponible
    Et la connexion "dbeaver-b" est listée comme indisponible
    Et la raison de la connexion "dbeaver-a" contient "connection file"

  Scénario: Un fichier qui porte déjà son indice n'y ajoute pas la proposition de le créer
    Étant donné le fichier de connexions suivant :
      """
      [[connection]]
      name = "a" "b"
      """
    Quand l'agent liste les connexions
    Alors l'indice cite la ligne 2
    Et l'indice ne contient pas "run `scarif-database-mcp connections import-dbeaver`"

  Scénario: Une valeur invalide de read_dbeaver rend les connexions DBeaver indisponibles
    Étant donné un espace de travail DBeaver où la connexion "canary-local" a son mot de passe enregistré
    Et le fichier de connexions suivant :
      """
      read_dbeaver = "no"
      """
    Quand l'agent liste les connexions
    Alors la connexion "canary-local" est listée comme indisponible
    Et la raison de la connexion "canary-local" contient "connection file"

  Scénario: Une clé de premier niveau qui porte un secret rend les connexions DBeaver indisponibles sans le recopier
    Étant donné un espace de travail DBeaver où la connexion "canary-local" a son mot de passe enregistré
    Et le fichier de connexions suivant :
      """
      password = "zq7x-top-level"
      """
    Quand l'agent liste les connexions
    Alors la connexion "canary-local" est listée comme indisponible
    Et la raison de la connexion "canary-local" contient "connection file"
    Et l'indice ne contient pas "zq7x-top-level"

  Scénario: Une erreur de syntaxe sur une ligne qui porte un mot de passe ne le recopie pas dans l'indice
    Étant donné le fichier de connexions suivant :
      """
      [[connection]]
      name = "a"
      password = "zq7x"k"
      """
    Quand l'agent liste les connexions
    Alors l'indice cite la ligne 3
    Et l'indice ne contient pas "zq7x"

  Scénario: Une connexion qui porte un mot de passe est indisponible
    Étant donné le fichier de connexions suivant :
      """
      [[connection]]
      name = "app-prod"
      host = "db.internal.example"
      user = "reader"
      type = "prod"
      auth = "pgpass"
      password = "reader-secret"
      """
    Quand l'agent liste les connexions
    Alors la connexion "app-prod" est listée comme indisponible
    Et la raison de la connexion "app-prod" contient "must not hold secrets"
    Et la liste ne montre ni hôte, ni utilisateur, ni mot de passe

  Scénario: Une clé inconnue rend la connexion indisponible avec une raison qui la nomme
    Étant donné le fichier de connexions suivant :
      """
      [[connection]]
      name = "app-prod"
      host = "db.internal.example"
      user = "reader"
      type = "prod"
      auth = "pgpass"
      colour = "blue"
      """
    Quand l'agent liste les connexions
    Alors la connexion "app-prod" est listée comme indisponible
    Et la raison de la connexion "app-prod" contient "colour"

  Scénario: Deux connexions du même nom sont toutes deux indisponibles
    Étant donné le fichier de connexions suivant :
      """
      [[connection]]
      name = "app"
      host = "db.internal.example"
      user = "reader"
      type = "dev"
      auth = "pgpass"

      [[connection]]
      name = "app"
      host = "db.internal.example"
      user = "reader"
      type = "dev"
      auth = "pgpass"
      """
    Quand l'agent liste les connexions
    Alors toutes les connexions nommées "app" sont indisponibles
    Et la raison de la connexion "app" contient "more than once"

  Scénario: Le réglage read_dbeaver à faux masque toutes les connexions DBeaver
    Étant donné un espace de travail DBeaver où la connexion "canary-local" a son mot de passe enregistré
    Et le fichier de connexions suivant :
      """
      read_dbeaver = false
      """
    Quand l'agent liste les connexions
    Alors la liste ne contient pas la connexion "canary-local"

  Scénario: Sans fichier ni DBeaver l'indice propose de créer le fichier ou de lancer l'import
    Étant donné aucun fichier de connexions
    Quand l'agent liste les connexions
    Alors l'indice propose de créer le fichier ou de lancer l'import

  @ae3
  Scénario: Une commande de mot de passe qui échoue rend un motif fixe avec son statut et sans sa sortie
    Étant donné le fichier de connexions suivant :
      """
      [[connection]]
      name = "app-prod"
      host = "db.internal.example"
      user = "reader"
      type = "prod"
      auth = "command"
      password_command = ["/bin/sh", "-c", "echo out-leak-4411; echo err-leak-4411 >&2; exit 44"]
      """
    Quand l'agent exécute une requête de lecture sur la connexion "app-prod"
    Alors la requête échoue avec une erreur qui contient "status 44"
    Et l'erreur de la requête ne contient pas "leak"

  Scénario: Lister les connexions n'exécute jamais la commande de mot de passe
    Étant donné le fichier de connexions suivant :
      """
      [[connection]]
      name = "app-prod"
      host = "db.internal.example"
      user = "reader"
      type = "prod"
      auth = "command"
      password_command = ["/bin/sh", "-c", "touch {dossier}/marqueur"]
      """
    Quand l'agent liste les connexions
    Alors la connexion "app-prod" est listée comme disponible
    Et la commande de mot de passe n'a pas été exécutée

  Scénario: Un alias SSH absent du fichier de configuration SSH rend la connexion indisponible avec une raison qui le nomme
    Étant donné un fichier de configuration SSH de test qui ne définit pas l'alias "bastion-absent"
    Et le fichier de connexions suivant :
      """
      [[connection]]
      name = "via-absent"
      host = "db.internal.example"
      user = "reader"
      type = "dev"
      auth = "command"
      password_command = ["/bin/echo", "reader-secret"]
      ssh_alias = "bastion-absent"
      """
    Quand l'agent liste les connexions
    Alors la connexion "via-absent" est listée comme indisponible
    Et la raison de la connexion "via-absent" contient "SSH alias bastion-absent is not defined in the OpenSSH config"
    Et la raison de la connexion "via-absent" contient "add a Host block"
    Et la liste ne montre ni hôte, ni utilisateur, ni mot de passe

  Scénario: Une connexion sans moteur déclaré est listée avec le moteur postgresql
    Étant donné le fichier de connexions suivant :
      """
      [[connection]]
      name = "app-prod"
      host = "db.internal.example"
      user = "reader"
      type = "prod"
      auth = "pgpass"
      """
    Quand l'agent liste les connexions
    Alors la connexion "app-prod" est listée avec le moteur "postgresql"

  Scénario: Une connexion MongoDB en mode commande est listée disponible avec son moteur sans exécuter la commande
    Étant donné le fichier de connexions suivant :
      """
      [[connection]]
      name = "atlas-dev"
      engine = "mongodb"
      host = "pl-0.atlas.scarif.test"
      user = "reader"
      type = "dev"
      auth = "command"
      password_command = ["/bin/sh", "-c", "touch {dossier}/marqueur"]
      """
    Quand l'agent liste les connexions
    Alors la connexion "atlas-dev" est listée comme disponible
    Et la connexion "atlas-dev" est listée avec le moteur "mongodb"
    Et la commande de mot de passe n'a pas été exécutée
    Et la liste ne montre ni hôte, ni utilisateur, ni mot de passe

  Scénario: Une connexion MongoDB sans SRV avec port, jeu de réplicas et TLS désactivé est listée disponible
    Étant donné le fichier de connexions suivant :
      """
      [[connection]]
      name = "atlas-local"
      engine = "mongodb"
      host = "pl-0.atlas.scarif.test"
      port = 1024
      srv = false
      tls = false
      replica_set = "rs0"
      database = "shop"
      user = "reader"
      type = "dev"
      auth = "command"
      password_command = ["/bin/echo", "reader-secret"]
      """
    Quand l'agent liste les connexions
    Alors la connexion "atlas-local" est listée comme disponible
    Et la connexion "atlas-local" est listée avec le moteur "mongodb"

  @ae11
  Scénario: Une connexion MongoDB en mode azure est indisponible avec une raison qui nomme auth
    Étant donné le fichier de connexions suivant :
      """
      [[connection]]
      name = "atlas-azure"
      engine = "mongodb"
      host = "pl-0.atlas.scarif.test"
      user = "reader"
      type = "dev"
      auth = "azure"
      """
    Quand l'agent liste les connexions
    Alors la connexion "atlas-azure" est listée comme indisponible
    Et la raison de la connexion "atlas-azure" contient "`auth`"

  Scénario: Une connexion MongoDB en mode pgpass est indisponible avec une raison qui nomme auth
    Étant donné le fichier de connexions suivant :
      """
      [[connection]]
      name = "atlas-pgpass"
      engine = "mongodb"
      host = "pl-0.atlas.scarif.test"
      user = "reader"
      type = "dev"
      auth = "pgpass"
      """
    Quand l'agent liste les connexions
    Alors la connexion "atlas-pgpass" est listée comme indisponible
    Et la raison de la connexion "atlas-pgpass" contient "`auth`"

  Scénario: Une clé propre à l'autre moteur rend la connexion indisponible avec une raison qui la nomme
    Étant donné le fichier de connexions suivant :
      """
      [[connection]]
      name = "atlas-sslmode"
      engine = "mongodb"
      host = "pl-0.atlas.scarif.test"
      user = "reader"
      type = "dev"
      auth = "command"
      password_command = ["/bin/echo", "reader-secret"]
      sslmode = "require"

      [[connection]]
      name = "pg-srv"
      host = "db.internal.example"
      user = "reader"
      type = "dev"
      auth = "pgpass"
      srv = true

      [[connection]]
      name = "pg-tls"
      host = "db.internal.example"
      user = "reader"
      type = "dev"
      auth = "pgpass"
      tls = true

      [[connection]]
      name = "pg-replica"
      host = "db.internal.example"
      user = "reader"
      type = "dev"
      auth = "pgpass"
      replica_set = "rs0"

      [[connection]]
      name = "atlas-tls-off"
      engine = "mongodb"
      host = "pl-0.atlas.scarif.test"
      tls = false
      user = "reader"
      type = "dev"
      auth = "command"
      password_command = ["/bin/echo", "reader-secret"]
      """
    Quand l'agent liste les connexions
    Alors la connexion "atlas-sslmode" est listée comme indisponible
    Et la raison de la connexion "atlas-sslmode" contient "`sslmode`"
    Et la connexion "pg-srv" est listée comme indisponible
    Et la raison de la connexion "pg-srv" contient "`srv`"
    Et la connexion "pg-tls" est listée comme indisponible
    Et la raison de la connexion "pg-tls" contient "`tls`"
    Et la connexion "pg-replica" est listée comme indisponible
    Et la raison de la connexion "pg-replica" contient "`replica_set`"
    Et la connexion "atlas-tls-off" est listée comme indisponible
    Et la raison de la connexion "atlas-tls-off" contient "`tls`"

  Scénario: Un moteur inconnu rend la connexion indisponible avec une raison qui nomme engine
    Étant donné le fichier de connexions suivant :
      """
      [[connection]]
      name = "app"
      engine = "mysql"
      host = "db.internal.example"
      user = "reader"
      type = "dev"
      auth = "pgpass"
      """
    Quand l'agent liste les connexions
    Alors la connexion "app" est listée comme indisponible
    Et la raison de la connexion "app" contient "`engine`"

  Scénario: Un outil qui ouvrirait une session sur une connexion MongoDB est refusé sans exécuter la commande ni joindre PostgreSQL
    Étant donné le fichier de connexions suivant :
      """
      [[connection]]
      name = "atlas-dev"
      engine = "mongodb"
      host = "pl-0.atlas.scarif.test"
      user = "reader"
      type = "dev"
      auth = "command"
      password_command = ["/bin/sh", "-c", "touch {dossier}/marqueur"]
      """
    Quand l'agent exécute une requête de lecture sur la connexion "atlas-dev"
    Alors la requête échoue avec une erreur qui contient "not supported"
    Et l'erreur de la requête ne contient pas "pl-0.atlas.scarif.test"
    Et la commande de mot de passe n'a pas été exécutée

  Scénario: Un port avec srv vrai rend la connexion MongoDB indisponible avec une raison qui nomme port
    Étant donné le fichier de connexions suivant :
      """
      [[connection]]
      name = "atlas-port"
      engine = "mongodb"
      host = "pl-0.atlas.scarif.test"
      port = 1024
      user = "reader"
      type = "dev"
      auth = "command"
      password_command = ["/bin/echo", "reader-secret"]
      """
    Quand l'agent liste les connexions
    Alors la connexion "atlas-port" est listée comme indisponible
    Et la raison de la connexion "atlas-port" contient "`port`"

  Scénario: La lecture du catalogue d'une connexion MongoDB est refusée sans exécuter la commande
    Étant donné le fichier de connexions suivant :
      """
      [[connection]]
      name = "atlas-dev"
      engine = "mongodb"
      host = "pl-0.atlas.scarif.test"
      user = "reader"
      type = "dev"
      auth = "command"
      password_command = ["/bin/sh", "-c", "touch {dossier}/marqueur"]
      """
    Quand l'agent lit le catalogue de la connexion "atlas-dev"
    Alors la requête échoue avec une erreur qui contient "not supported"
    Et la commande de mot de passe n'a pas été exécutée
