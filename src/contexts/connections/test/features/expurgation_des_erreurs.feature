# language: fr
@connections
Fonctionnalité: Erreurs de connexion sans identité
  Une erreur de connexion rendue à l'agent ne nomme ni l'hôte ni l'utilisateur de la base, ni ceux des bastions traversés : ils deviennent [redacted].
  Le reste du message, comme la cause réseau ou le code du serveur, est conservé pour que l'agent comprenne l'échec.

  Règle: Une erreur de connexion PostgreSQL perd l'hôte et l'utilisateur de la base et de chaque saut SSH

    Plan du Scénario: Le message « <message> » est rendu « <rendu> »
      Étant donné une connexion PostgreSQL vers l'hôte "db-interne.example.com" en tant que "alice", par les sauts SSH "bastion.example.com" en tant que "jump" puis "relais.example.com" en tant que "relay"
      Quand la connexion échoue en <nature> avec le message « <message> »
      Alors l'erreur rendue est « <rendu> »

      Exemples:
        | nature            | message                                                       | rendu                                                   |
        | authentification  | password authentication failed for user "alice"               | password authentication failed for user "[redacted]"    |
        | connexion         | could not reach db-interne.example.com:5432                   | could not reach [redacted]:5432                         |
        | authentification  | SSH authentication of jump@bastion.example.com was rejected   | SSH authentication of [redacted]@[redacted] was rejected |
        | connexion         | relais.example.com refused to forward to db-interne.example.com:5432 | [redacted] refused to forward to [redacted]:5432 |
        | connexion         | SSH user relay is not allowed                                 | SSH user [redacted] is not allowed                      |
        | connexion         | Connection refused (os error 61)                              | Connection refused (os error 61)                        |

  Règle: Une erreur MongoDB perd le nom du cluster, ses membres, l'utilisateur, le bastion et les identifiants du front SOCKS5

    Plan du Scénario: Le message MongoDB « <message> » est rendu « <rendu> »
      Étant donné une connexion MongoDB vers le nom SRV "cluster0.example.com" en tant que "alice", par le saut SSH "bastion.example.com" en tant que "jump", avec un front SOCKS5
      Quand le pilote MongoDB échoue avec le message « <message> »
      Alors l'erreur rendue est « <rendu> »

      Exemples:
        | message                                                     | rendu                                                  |
        | no server found for cluster0.example.com                    | no server found for [redacted]                         |
        | shard-00-01.example.com:27017 is unreachable                | [redacted]:27017 is unreachable                        |
        | user alice is not authorized                                | user [redacted] is not authorized                      |
        | tunnel through bastion.example.com as jump closed           | tunnel through [redacted] as [redacted] closed         |
        | proxy refused {identifiants du front}                       | proxy refused [redacted]:[redacted]                    |
        | other.example.org is not a member                           | other.example.org is not a member                      |
