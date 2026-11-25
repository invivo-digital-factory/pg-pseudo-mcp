# language: fr
@connections
Fonctionnalité: Jeton Azure pour Azure Database for PostgreSQL
  En mode azure, le mot de passe est le jeton d'accès Entra ID que délivre l'Azure CLI (az account get-access-token), demandé par le serveur lui-même.
  Ce jeton vaut pour tous les serveurs Azure PostgreSQL de l'utilisateur : il n'est envoyé qu'à un hôte en .postgres.database.azure.com, et toujours sous TLS.

  Règle: Le mode azure n'est accepté que sur un hôte Azure PostgreSQL, et az n'est jamais lancé pour un autre hôte

    Plan du Scénario: L'hôte <hôte> reçoit le jeton Azure
      Étant donné un Azure CLI connecté qui délivre le jeton "entra-jeton-1"
      Et une connexion du fichier "shop" en mode azure vers l'hôte "<hôte>"
      Quand l'agent liste les connexions
      Alors la connexion "shop" est listée comme disponible
      Et le mot de passe retenu pour la connexion "shop" est « entra-jeton-1 »

      Exemples:
        | hôte                                         |
        | shop.postgres.database.azure.com             |
        | shop.privatelink.postgres.database.azure.com |
        | Shop.Postgres.Database.Azure.COM.            |

    Plan du Scénario: L'hôte <hôte> est refusé sans lancer az
      Étant donné un Azure CLI connecté qui délivre le jeton "entra-jeton-1"
      Et une connexion du fichier "shop" en mode azure vers l'hôte "<hôte>"
      Quand l'agent liste les connexions
      Alors la connexion "shop" est listée comme indisponible
      Et la raison de la connexion "shop" contient "the Azure token is only sent to Azure PostgreSQL hosts"
      Et l'Azure CLI n'a pas été lancé

      Exemples:
        | hôte                                          |
        | db.example.com                                |
        | postgres.database.azure.com                   |
        | shop.postgres.database.azure.com.example.com  |
        | shop.mysql.database.azure.com                 |

  Règle: Le jeton Azure prime sur une entrée pgpass, qui ne sert que de repli quand az échoue

    Scénario: Le jeton de az est retenu même quand pgpass a une entrée pour le serveur
      Étant donné un Azure CLI connecté qui délivre le jeton "entra-jeton-1"
      Et un fichier pgpass qui contient :
        """
        *:*:*:alice@example.com:depuis-pgpass
        """
      Et une connexion du fichier "shop" en mode azure vers l'hôte "shop.postgres.database.azure.com"
      Quand l'agent liste les connexions
      Alors le mot de passe retenu pour la connexion "shop" est « entra-jeton-1 »

    Scénario: Quand az demande une connexion, l'entrée pgpass correspondante sert de repli
      Étant donné un Azure CLI qui demande de se connecter avec le message "alice@example.com must run az login"
      Et un fichier pgpass qui contient :
        """
        *:*:*:alice@example.com:depuis-pgpass
        """
      Et une connexion du fichier "shop" en mode azure vers l'hôte "shop.postgres.database.azure.com"
      Quand l'agent liste les connexions
      Alors la connexion "shop" est listée comme disponible
      Et le mot de passe retenu pour la connexion "shop" est « depuis-pgpass »

    Plan du Scénario: Sans repli pgpass, un Azure CLI <état> rend la connexion indisponible avec la marche à suivre
      Étant donné un Azure CLI <état>
      Et une connexion du fichier "shop" en mode azure vers l'hôte "shop.postgres.database.azure.com"
      Quand l'agent liste les connexions
      Alors la connexion "shop" est listée comme indisponible
      Et la raison de la connexion "shop" contient "<raison>"

      Exemples:
        | état                                                                          | raison                                                                  |
        | qui demande de se connecter avec le message "alice@example.com must run az login" | no Azure access token available, run az login                       |
        | absent du poste                                                               | the Azure CLI (az) was not found on PATH; install it, then run az login |
        | qui répond "jeton-illisible"                                                  | the Azure CLI answered with an access token that cannot be read        |

    Plan du Scénario: Ce qu'écrit un Azure CLI <état> n'est jamais recopié dans la liste
      Étant donné un Azure CLI <état>
      Et une connexion du fichier "shop" en mode azure vers l'hôte "shop.postgres.database.azure.com"
      Quand l'agent liste les connexions
      Alors la connexion "shop" est listée comme indisponible
      Et la liste ne montre ni l'hôte "shop.postgres.database.azure.com", ni l'utilisateur "alice@example.com", ni le secret "<sortie>"

      Exemples:
        | état                                                                              | sortie            |
        | qui demande de se connecter avec le message "alice@example.com must run az login" | must run az login |
        | qui répond "jeton-illisible"                                                      | jeton-illisible   |

  Règle: Une connexion qui reçoit le jeton Azure passe au moins en TLS require

    Plan du Scénario: Le mode TLS <déclaré> devient <retenu> avec le jeton Azure
      Étant donné un Azure CLI connecté qui délivre le jeton "entra-jeton-1"
      Et une connexion du fichier "shop" en mode azure vers l'hôte "shop.postgres.database.azure.com" avec le mode TLS "<déclaré>"
      Quand l'agent liste les connexions
      Alors la connexion "shop" vise le port 5432 avec le mode TLS "<retenu>"

      Exemples:
        | déclaré     | retenu      |
        | disable     | require     |
        | allow       | require     |
        | prefer      | require     |
        | require     | require     |
        | verify-ca   | verify-ca   |
        | verify-full | verify-full |

  Règle: Un jeton obtenu sert toutes les connexions Azure tant qu'il reste valide

    Scénario: Deux connexions Azure listées deux fois ne lancent az qu'une fois
      Étant donné un Azure CLI connecté qui délivre le jeton "entra-jeton-1"
      Et le fichier de connexions suivant :
        """
        [[connection]]
        name = "shop"
        host = "shop.postgres.database.azure.com"
        user = "alice@example.com"
        type = "dev"
        auth = "azure"

        [[connection]]
        name = "billing"
        host = "billing.postgres.database.azure.com"
        user = "alice@example.com"
        type = "dev"
        auth = "azure"
        """
      Quand l'agent liste les connexions
      Alors la connexion "shop" est listée comme disponible
      Et la connexion "billing" est listée comme disponible
      Quand l'agent liste les connexions
      Alors la connexion "shop" est listée comme disponible
      Et l'Azure CLI a été lancé 1 fois

  Règle: Dans DBeaver, l'authentification pgpass sur un hôte Azure prend le jeton, l'authentification native jamais

    Plan du Scénario: Une connexion DBeaver <modèle> sur un hôte Azure <usage>
      Étant donné un Azure CLI connecté qui délivre le jeton "entra-jeton-1"
      Et un espace de travail DBeaver avec une connexion "shop" au modèle d'authentification "<modèle>" vers l'hôte "shop.postgres.database.azure.com"
      Quand l'agent liste les connexions
      Alors le mot de passe retenu pour la connexion "shop" est « <mot de passe> »
      Et l'Azure CLI a été lancé <lancements> fois

      Exemples:
        | modèle          | usage                       | mot de passe     | lancements |
        | postgres_pgpass | prend le jeton de az        | entra-jeton-1    | 1          |
        | native          | garde son mot de passe      | mot-de-passe-dbeaver | 0      |
