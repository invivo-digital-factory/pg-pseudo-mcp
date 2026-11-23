# language: fr
@connections
Fonctionnalité: Fichier de connexions
  L'agent lit les connexions du fichier propre à pg-pseudo, qui ne contient aucun secret, et garde DBeaver comme source de repli.

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
