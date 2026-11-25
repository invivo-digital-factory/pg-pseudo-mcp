# language: fr
@connections
Fonctionnalité: Mot de passe lu dans le fichier pgpass
  En mode pgpass, le mot de passe vient du fichier pgpass, comme pour psql : la première ligne dont l'hôte, le port, la base et l'utilisateur correspondent l'emporte.
  L'hôte cherché est celui de la base tel qu'écrit dans le fichier de connexions, jamais celui du bastion SSH, et un fichier lisible par d'autres est refusé.

  Règle: La première ligne dont les quatre champs correspondent donne le mot de passe

    Plan du Scénario: La ligne pgpass « <ligne> » donne le mot de passe « <mot de passe> »
      Étant donné un fichier pgpass qui contient :
        """
        <ligne>
        *:*:*:alice:repli
        """
      Et le fichier de connexions suivant :
        """
        [[connection]]
        name = "shop"
        host = "db.example.com"
        database = "shop"
        user = "alice"
        type = "dev"
        auth = "pgpass"
        """
      Quand l'agent liste les connexions
      Alors la connexion "shop" est listée comme disponible
      Et le mot de passe retenu pour la connexion "shop" est « <mot de passe> »

      Exemples: Lignes qui correspondent
        | ligne                                    | mot de passe |
        | db.example.com:5432:shop:alice:s3cret    | s3cret       |
        | *:5432:shop:alice:s3cret                 | s3cret       |
        | db.example.com:*:*:alice:s3cret          | s3cret       |

      Exemples: Lignes qui ne correspondent pas, la ligne de repli suivante l'emporte
        | ligne                                    | mot de passe |
        | db.example.com:5432:billing:alice:s3cret | repli        |
        | db.example.com:6432:shop:alice:s3cret    | repli        |
        | other.example.com:5432:shop:alice:s3cret | repli        |
        | db.example.com:5432:shop:bob:s3cret      | repli        |

    Scénario: Les commentaires, les lignes vides et les lignes incomplètes sont ignorés
      Étant donné un fichier pgpass qui contient :
        """
        # db.example.com:5432:shop:alice:en-commentaire

        db.example.com:5432:shop:alice
        db.example.com:5432:shop:alice:s3cret
        """
      Et le fichier de connexions suivant :
        """
        [[connection]]
        name = "shop"
        host = "db.example.com"
        database = "shop"
        user = "alice"
        type = "dev"
        auth = "pgpass"
        """
      Quand l'agent liste les connexions
      Alors le mot de passe retenu pour la connexion "shop" est « s3cret »

    Scénario: Un deux-points ou une barre oblique inverse échappés font partie du mot de passe
      Étant donné un fichier pgpass qui contient :
        """
        db.example.com:5432:shop:alice:s3\:cr\\et
        """
      Et le fichier de connexions suivant :
        """
        [[connection]]
        name = "shop"
        host = "db.example.com"
        database = "shop"
        user = "alice"
        type = "dev"
        auth = "pgpass"
        """
      Quand l'agent liste les connexions
      Alors le mot de passe retenu pour la connexion "shop" est « s3:cr\et »

    Scénario: Une étoile précédée d'une barre oblique inverse est littérale
      Étant donné un fichier pgpass qui contient :
        """
        \*:5432:shop:alice:etoile-litterale
        *:*:*:alice:repli
        """
      Et le fichier de connexions suivant :
        """
        [[connection]]
        name = "shop"
        host = "db.example.com"
        database = "shop"
        user = "alice"
        type = "dev"
        auth = "pgpass"
        """
      Quand l'agent liste les connexions
      Alors le mot de passe retenu pour la connexion "shop" est « repli »

  Règle: Sans ligne correspondante ni fichier pgpass, la connexion est indisponible

    Scénario: Un fichier pgpass sans ligne pour la base rend la connexion indisponible
      Étant donné un fichier pgpass qui contient :
        """
        db.example.com:5432:billing:alice:s3cret
        """
      Et le fichier de connexions suivant :
        """
        [[connection]]
        name = "shop"
        host = "db.example.com"
        database = "shop"
        user = "alice"
        type = "dev"
        auth = "pgpass"
        """
      Quand l'agent liste les connexions
      Alors la connexion "shop" est listée comme indisponible
      Et la raison de la connexion "shop" contient "no matching entry in the pgpass file"
      Et la liste ne montre ni l'hôte "db.example.com", ni l'utilisateur "alice", ni le secret "s3cret"

    Scénario: Sans fichier pgpass la connexion est indisponible
      Étant donné le fichier de connexions suivant :
        """
        [[connection]]
        name = "shop"
        host = "db.example.com"
        user = "alice"
        type = "dev"
        auth = "pgpass"
        """
      Quand l'agent liste les connexions
      Alors la connexion "shop" est listée comme indisponible
      Et la raison de la connexion "shop" contient "no matching entry in the pgpass file"

  Règle: Un fichier pgpass lisible par le groupe ou les autres, ou qui n'est pas un fichier, est refusé

    Plan du Scénario: Un fichier pgpass <état> est refusé
      Étant donné un fichier pgpass <état> qui contient :
        """
        *:*:*:alice:s3cret
        """
      Et le fichier de connexions suivant :
        """
        [[connection]]
        name = "shop"
        host = "db.example.com"
        user = "alice"
        type = "dev"
        auth = "pgpass"
        """
      Quand l'agent liste les connexions
      Alors la connexion "shop" est listée comme indisponible
      Et la raison de la connexion "shop" contient "<raison>"

      Exemples:
        | état                     | raison                                                     |
        | aux droits 644           | pgpass file permissions are too open; run chmod 600 on it  |
        | aux droits 640           | pgpass file permissions are too open; run chmod 600 on it  |
        | remplacé par un dossier  | pgpass file is not a regular file                          |

  Règle: L'hôte cherché est celui de la base tel qu'écrit dans le fichier, même derrière un alias SSH

    Scénario: Une connexion derrière un bastion trouve sa ligne pgpass par l'hôte de la base
      Étant donné un fichier de configuration SSH qui définit l'alias "bastion" vers "bastion.example.com"
      Et un fichier pgpass qui contient :
        """
        bastion.example.com:5432:shop:alice:mauvais
        db.example.com:5432:shop:alice:s3cret
        """
      Et le fichier de connexions suivant :
        """
        [[connection]]
        name = "shop"
        host = "db.example.com"
        database = "shop"
        user = "alice"
        type = "dev"
        auth = "pgpass"
        ssh_alias = "bastion"
        """
      Quand l'agent liste les connexions
      Alors la connexion "shop" est listée comme disponible
      Et le mot de passe retenu pour la connexion "shop" est « s3cret »

  Règle: Une autre base demandée par l'argument database est aussi celle cherchée dans pgpass

    Plan du Scénario: La base <base> trouve sa propre ligne pgpass
      Étant donné un fichier pgpass qui contient :
        """
        db.example.com:5432:shop:alice:pour-shop
        db.example.com:5432:billing:alice:pour-billing
        """
      Et <source> qui déclare la connexion "shop" vers la base "shop" en mode pgpass
      Quand le serveur prépare la connexion "shop" pour la base "<base>"
      Alors le mot de passe retenu pour la connexion "shop" est « <mot de passe> »

      Exemples:
        | source                                | base    | mot de passe |
        | le fichier de connexions              | billing | pour-billing |
        | le fichier de connexions              | shop    | pour-shop    |
        | un espace de travail DBeaver          | billing | pour-billing |

  Règle: Un mot de passe pgpass garde le mode TLS déclaré

    Scénario: Le mode TLS prefer est conservé hors Azure
      Étant donné un fichier pgpass qui contient :
        """
        *:*:*:alice:s3cret
        """
      Et le fichier de connexions suivant :
        """
        [[connection]]
        name = "shop"
        host = "db.example.com"
        user = "alice"
        type = "dev"
        auth = "pgpass"
        """
      Quand l'agent liste les connexions
      Alors la connexion "shop" vise le port 5432 avec le mode TLS "prefer"
