# language: fr
@connections
Fonctionnalité: Priorité du fichier sur DBeaver
  Le fichier de connexions est la source de référence, et DBeaver reste lu en repli tant que le fichier ne le coupe pas.
  Une connexion DBeaver dont le nom est déclaré dans le fichier n'est ni listée ni utilisable : la source de chaque connexion est affichée.

  Règle: Une connexion du fichier masque la connexion DBeaver du même nom, même quand l'entrée du fichier est indisponible

    @ae1
    Scénario: Une connexion du même nom dans le fichier et dans DBeaver n'est listée qu'une fois
      Étant donné un espace de travail DBeaver où la connexion "canary-local" a son mot de passe enregistré
      Et le fichier de connexions suivant :
        """
        [[connection]]
        name = "canary-local"
        host = "db.example.com"
        user = "alice"
        type = "dev"
        auth = "command"
        password_command = ["/bin/echo", "s3cret"]
        """
      Quand l'agent liste les connexions
      Alors la connexion "canary-local" n'est listée qu'une seule fois
      Et la connexion "canary-local" est listée avec la source "file"

    Scénario: Une entrée du fichier indisponible masque quand même la connexion DBeaver
      Étant donné un espace de travail DBeaver où la connexion "canary-local" a son mot de passe enregistré
      Et le fichier de connexions suivant :
        """
        [[connection]]
        name = "canary-local"
        host = "db.example.com"
        user = "alice"
        type = "dev"
        auth = "pgpass"
        """
      Quand l'agent liste les connexions
      Alors la connexion "canary-local" n'est listée qu'une seule fois
      Et la connexion "canary-local" est listée avec la source "file"
      Et la connexion "canary-local" est listée comme indisponible

    Scénario: Les connexions DBeaver d'un autre nom restent listées avec leur source
      Étant donné un espace de travail DBeaver où la connexion "canary-local" a son mot de passe enregistré
      Et le fichier de connexions suivant :
        """
        [[connection]]
        name = "app"
        host = "db.example.com"
        user = "alice"
        type = "dev"
        auth = "command"
        password_command = ["/bin/echo", "s3cret"]
        """
      Quand l'agent liste les connexions
      Alors la connexion "app" est listée avec la source "file"
      Et la connexion "canary-local" est listée avec la source "dbeaver"
      Et la connexion "canary-local" est listée comme disponible

  Règle: Un nom du fichier masque tous les doublons DBeaver de ce nom, un nom suffixé n'en masque qu'un

    Scénario: Le nom écrit dans DBeaver masque ses deux doublons
      Étant donné un espace de travail DBeaver avec deux connexions nommées "app"
      Et le fichier de connexions suivant :
        """
        [[connection]]
        name = "app"
        host = "db.example.com"
        user = "alice"
        type = "dev"
        auth = "command"
        password_command = ["/bin/echo", "s3cret"]
        """
      Quand l'agent liste les connexions
      Alors la connexion "app" est listée avec la source "file"
      Et aucune connexion de la source "dbeaver" n'est listée

    Scénario: Le nom suffixé d'un doublon DBeaver ne masque que ce doublon
      Étant donné un espace de travail DBeaver avec deux connexions nommées "app"
      Et un fichier de connexions qui déclare la première connexion DBeaver "app" sous son nom suffixé
      Quand l'agent liste les connexions
      Alors une seule connexion de la source "dbeaver" est listée
      Et le doublon déclaré dans le fichier est listé avec la source "file"

  Règle: read_dbeaver = false coupe DBeaver, même quand le fichier a un autre problème

    Scénario: Le réglage read_dbeaver à faux masque toutes les connexions DBeaver
      Étant donné un espace de travail DBeaver où la connexion "canary-local" a son mot de passe enregistré
      Et le fichier de connexions suivant :
        """
        read_dbeaver = false
        """
      Quand l'agent liste les connexions
      Alors la liste ne contient pas la connexion "canary-local"

    Scénario: read_dbeaver à faux l'emporte sur une clé de premier niveau inconnue
      Étant donné un espace de travail DBeaver où la connexion "canary-local" a son mot de passe enregistré
      Et le fichier de connexions suivant :
        """
        read_dbeaver = false
        colour = "blue"
        """
      Quand l'agent liste les connexions
      Alors la liste ne contient pas la connexion "canary-local"
      Et l'indice contient "unknown top-level key `colour`"

  Règle: L'indice de DBeaver n'apparaît que lorsqu'aucune connexion n'est listée

    Scénario: Sans espace de travail DBeaver, une connexion du fichier suffit à taire l'indice
      Étant donné le fichier de connexions suivant :
        """
        [[connection]]
        name = "app"
        host = "db.example.com"
        user = "alice"
        type = "dev"
        auth = "command"
        password_command = ["/bin/echo", "s3cret"]
        """
      Quand l'agent liste les connexions
      Alors la liste ne porte aucun indice

    Scénario: Sans aucune connexion, l'indice dit qu'aucun espace DBeaver n'a été trouvé et propose l'import
      Étant donné le fichier de connexions suivant :
        """
        read_dbeaver = true
        """
      Quand l'agent liste les connexions
      Alors l'indice contient "no DBeaver workspace found"
      Et l'indice contient "no connection found"
      Et l'indice propose de créer le fichier ou de lancer l'import
