# language: fr
@connections
Fonctionnalité: Front SOCKS5 vers les membres d'un cluster MongoDB
  Pour un cluster privé derrière un bastion, le serveur ouvre un front SOCKS5 local par lequel le pilote MongoDB joint chaque membre, à travers le tunnel SSH et par son nom.
  Ce front n'écoute que sur la boucle locale, exige des identifiants propres à la session et ne relaie que les noms du domaine du cluster.

  Règle: Avec un nom SRV, seuls les membres du domaine parent de ce nom sont admis

    Plan du Scénario: La cible <cible> est <verdict> pour le nom SRV cluster0.example.com
      Étant donné la liste d'admission du nom SRV "cluster0.example.com"
      Quand le front reçoit une demande vers "<cible>" sur le port <port>
      Alors la cible est <verdict>

      Exemples: Membres admis, transmis en minuscules
        | cible                         | port  | verdict                                        |
        | shard-00-00.example.com       | 27017 | admise sous le nom "shard-00-00.example.com"   |
        | Shard-00-01.EXAMPLE.com       | 27017 | admise sous le nom "shard-00-01.example.com"   |
        | a.b.example.com               | 27017 | admise sous le nom "a.b.example.com"           |

      Exemples: Cibles refusées
        | cible                              | port  | verdict |
        | example.com                        | 27017 | refusée |
        | shard.example.org                  | 27017 | refusée |
        | shard-00-00.example.com.evil.test  | 27017 | refusée |
        | shardexample.com                   | 27017 | refusée |
        | 10.0.0.5                           | 27017 | refusée |
        | shard-00-00.example.com.           | 27017 | refusée |
        | -shard.example.com                 | 27017 | refusée |
        | shard_00.example.com               | 27017 | refusée |
        | shard-00-00.example.com            | 0     | refusée |

    Scénario: Un label de plus de 63 caractères est refusé
      Étant donné la liste d'admission du nom SRV "cluster0.example.com"
      Quand le front reçoit une demande vers un membre dont un label compte 64 caractères
      Alors la cible est refusée

    Plan du Scénario: Le nom SRV « <nom> » est refusé
      Quand le serveur prépare la liste d'admission du nom SRV "<nom>"
      Alors la liste d'admission est refusée avec le motif « <motif> »

      Exemples:
        | nom          | motif                                          |
        | example.com  | the MongoDB SRV name needs at least three labels |
        | localhost    | the MongoDB SRV name needs at least three labels |
        | 10.0.0.5     | the MongoDB host is not a DNS host name        |
        | exa mple.com | the MongoDB host is not a DNS host name        |

  Règle: Sans SRV, le domaine parent de l'hôte déclaré est admis, et un hôte à moins de trois labels n'admet que lui-même

    Plan du Scénario: Avec l'hôte déclaré <hôte>, la cible <cible> est <verdict>
      Étant donné la liste d'admission de l'hôte déclaré "<hôte>"
      Quand le front reçoit une demande vers "<cible>" sur le port 27017
      Alors la cible est <verdict>

      Exemples:
        | hôte                | cible               | verdict                                    |
        | mongo-0.example.com | mongo-1.example.com | admise sous le nom "mongo-1.example.com"   |
        | mongo-0.example.com | mongo-0.example.com | admise sous le nom "mongo-0.example.com"   |
        | mongo-0.example.com | mongo.example.org   | refusée                                    |
        | mongo.local         | mongo.local         | admise sous le nom "mongo.local"           |
        | mongo.local         | other.local         | refusée                                    |
        | localhost           | localhost           | admise sous le nom "localhost"             |

  Règle: Le front n'écoute que sur la boucle locale, avec des identifiants aléatoires propres à chaque session

    Scénario: Deux fronts écoutent sur 127.0.0.1 avec des identifiants différents
      Étant donné deux fronts SOCKS5 démarrés pour le membre "mongo-0.example.com"
      Alors chaque front écoute sur 127.0.0.1
      Et les deux fronts ont des identifiants différents

    Plan du Scénario: Un client <client> est refusé à la poignée de main sans ouvrir de canal
      Étant donné un front SOCKS5 démarré pour le membre "mongo-0.example.com"
      Quand un client <client> se présente au front
      Alors le front refuse le client
      Et aucun canal n'a été ouvert vers le bastion

      Exemples:
        | client                                      |
        | qui ne propose pas d'authentification       |
        | qui présente d'autres identifiants          |

  Règle: Une demande hors liste, vers une adresse IP ou autre que CONNECT est refusée sans ouvrir de canal ; une demande admise est relayée par un canal du bastion

    Plan du Scénario: Une demande <demande> est refusée sans ouvrir de canal
      Étant donné un front SOCKS5 démarré pour le membre "mongo-0.example.com"
      Quand un client authentifié demande <demande>
      Alors le front répond par le refus « <réponse> »
      Et aucun canal n'a été ouvert vers le bastion

      Exemples:
        | demande                                       | réponse                  |
        | CONNECT vers le nom "mongo.example.org"       | connexion non autorisée  |
        | CONNECT vers l'adresse IPv4 10.0.0.5          | connexion non autorisée  |
        | CONNECT vers l'adresse IPv6 ::1               | connexion non autorisée  |
        | BIND vers le nom "mongo-1.example.com"        | commande non prise en charge |
        | UDP ASSOCIATE vers le nom "mongo-1.example.com" | commande non prise en charge |

    Scénario: Une demande admise est relayée vers le nom vérifié, en minuscules
      Étant donné un front SOCKS5 démarré pour le membre "mongo-0.example.com"
      Quand un client authentifié demande CONNECT vers le nom "Mongo-1.Example.com"
      Alors le front répond par un succès
      Et un canal a été ouvert vers "mongo-1.example.com" sur le port 27017
      Et les octets envoyés par le client lui reviennent par ce canal

    Scénario: Un canal refusé par le bastion rend un échec général
      Étant donné un front SOCKS5 démarré pour le membre "mongo-0.example.com" derrière un bastion qui refuse les canaux
      Quand un client authentifié demande CONNECT vers le nom "mongo-1.example.com"
      Alors le front répond par le refus « échec général »
