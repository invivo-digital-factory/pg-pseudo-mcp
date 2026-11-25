# language: fr
@connections
Fonctionnalité: Import des connexions DBeaver
  Un utilisateur de DBeaver génère une fois son fichier de connexions avec la commande connections import-dbeaver, sans qu'aucun secret ne soit écrit.
  Le résumé de l'import dit, connexion par connexion, ce qu'il reste à faire : entrée pgpass ou commande, bloc Host SSH, clés d'hôte.

  Règle: L'import écrit une entrée par connexion PostgreSQL, sans aucun mot de passe

    Scénario: L'import écrit une connexion par connexion PostgreSQL sans le mot de passe enregistré dans DBeaver
      Étant donné un espace de travail DBeaver avec une connexion à mot de passe natif enregistré
      Quand l'utilisateur lance l'import des connexions DBeaver
      Alors le fichier de connexions déclare la connexion "canary-local"
      Et le fichier de connexions ne contient pas le mot de passe enregistré dans DBeaver
      Et le résumé annonce que le fichier prime désormais sur DBeaver

    Scénario: Le fichier importé n'est lisible que par son propriétaire et le serveur le relit sans erreur
      Étant donné un espace de travail DBeaver avec une connexion à mot de passe natif enregistré
      Quand l'utilisateur lance l'import des connexions DBeaver
      Et l'agent liste les connexions
      Alors le fichier de connexions a les droits 600
      Et la connexion "canary-local" est listée avec la source "file"

    Scénario: Sans connexion PostgreSQL l'import échoue sans rien écrire
      Étant donné un espace de travail DBeaver avec une connexion MySQL seulement
      Quand l'utilisateur lance l'import des connexions DBeaver
      Alors l'import échoue avec le message "DBeaver has no PostgreSQL connection to import"
      Et aucun fichier de connexions n'a été écrit

  Règle: Un fichier existant n'est remplacé qu'avec l'option --force

    @ae7
    Scénario: Un import sans option refuse d'écraser un fichier existant
      Étant donné un espace de travail DBeaver avec une connexion à mot de passe natif enregistré
      Et un fichier de connexions existant
      Quand l'utilisateur lance l'import des connexions DBeaver
      Alors l'import échoue en demandant l'option de remplacement
      Et le fichier de connexions existant est inchangé

    Scénario: Un import avec remplacement écrit le nouveau fichier
      Étant donné un espace de travail DBeaver avec une connexion à mot de passe natif enregistré
      Et un fichier de connexions existant
      Quand l'utilisateur lance l'import des connexions DBeaver avec remplacement
      Alors le fichier de connexions déclare la connexion "canary-local"

  Règle: L'authentification est traduite, et le type inconnu devient prod, le plus prudent

    @ae8
    Scénario: Une connexion native devient pgpass et le résumé demande une entrée pgpass ou une commande
      Étant donné un espace de travail DBeaver avec une connexion à mot de passe natif enregistré
      Quand l'utilisateur lance l'import des connexions DBeaver
      Alors la connexion "canary-local" utilise l'authentification "pgpass"
      Et le résumé demande une entrée pgpass ou une commande de mot de passe

    Scénario: Une connexion pgpass sur un hôte Azure devient azure et le résumé demande az login
      Étant donné un espace de travail DBeaver avec une connexion pgpass sur un serveur Azure
      Quand l'utilisateur lance l'import des connexions DBeaver
      Alors la connexion "azure-shop" utilise l'authentification "azure"
      Et le résumé contient "run az login: the Azure token is used as the password"

    Scénario: Un type DBeaver autre que dev, test ou prod est importé en prod et le résumé le signale
      Étant donné un espace de travail DBeaver avec une connexion "staging-db" de type "staging"
      Quand l'utilisateur lance l'import des connexions DBeaver
      Alors la connexion "staging-db" est importée avec le type "prod"
      Et le résumé contient "is not dev, test or prod: imported as prod"

  Règle: Un tunnel SSH devient un alias SSH, réutilisé s'il existe déjà, sinon proposé en bloc Host sans jamais modifier la configuration SSH

    Scénario: Une connexion à tunnel SSH reçoit un alias et le résumé propose un bloc Host
      Étant donné un espace de travail DBeaver avec une connexion qui traverse un bastion SSH
      Quand l'utilisateur lance l'import des connexions DBeaver
      Alors la connexion "canary-local" porte l'alias SSH "bastion.corp.example"
      Et le résumé propose un bloc Host avec l'hôte, le port, l'utilisateur et le fichier de clé
      Et le résumé rappelle d'ajouter les clés d'hôte de "bastion.corp.example" à known_hosts
      Et le fichier de configuration SSH est inchangé

    Scénario: Un alias de la configuration SSH qui mène au même bastion est réutilisé sans bloc proposé
      Étant donné un espace de travail DBeaver avec une connexion qui traverse un bastion SSH
      Et un fichier de configuration SSH dont l'alias "corp-bastion" mène à "bastion.corp.example" sur le port 2222 avec l'utilisateur "jump"
      Quand l'utilisateur lance l'import des connexions DBeaver
      Alors la connexion "canary-local" porte l'alias SSH "corp-bastion"
      Et le résumé contient "ssh alias corp-bastion already exists in ~/.ssh/config"
      Et le résumé ne propose aucun bloc Host
      Et le fichier de configuration SSH est inchangé

    Scénario: Plusieurs connexions derrière le même bastion partagent un seul alias et un seul bloc
      Étant donné un espace de travail DBeaver avec deux connexions "app-a" et "app-b" derrière le bastion "bastion.example.com"
      Quand l'utilisateur lance l'import des connexions DBeaver
      Alors la connexion "app-a" porte l'alias SSH "bastion.example.com"
      Et la connexion "app-b" porte l'alias SSH "bastion.example.com"
      Et le résumé propose exactement 1 bloc Host
      Et le fichier de configuration SSH est inchangé

    Scénario: L'alias proposé évite un alias déjà défini
      Étant donné un espace de travail DBeaver avec deux connexions "app-a" et "app-b" derrière le bastion "bastion.example.com"
      Et un fichier de configuration SSH dont l'alias "bastion.example.com" mène à "other.example.com" sur le port 22 avec l'utilisateur "alice"
      Quand l'utilisateur lance l'import des connexions DBeaver
      Alors la connexion "app-a" porte l'alias SSH "bastion.example.com-2"
      Et le résumé propose exactement 1 bloc Host
      Et le fichier de configuration SSH est inchangé

    Plan du Scénario: Un tunnel dont <champ> ne peut pas s'écrire sans risque dans la configuration SSH est écarté
      Étant donné un espace de travail DBeaver avec la connexion "kept" et la connexion "risky" derrière le bastion "<hôte>" avec l'utilisateur SSH "<utilisateur>"
      Quand l'utilisateur lance l'import des connexions DBeaver
      Alors le résumé ignore la connexion "risky" avec la raison "cannot be written to an OpenSSH config safely"
      Et le résumé ne propose aucun bloc Host
      Et le fichier de connexions déclare la connexion "kept"

      Exemples:
        | champ            | hôte                | utilisateur |
        | l'hôte           | -oProxyCommand=x    | jump        |
        | l'utilisateur    | bastion.example.com | j ump       |

  Règle: Les connexions que le serveur ne sait pas utiliser sont ignorées avec leur raison

    Scénario: Les connexions MySQL et à serveur de rebond sont ignorées avec leur raison
      Étant donné un espace de travail DBeaver avec une connexion MySQL et une connexion à serveur de rebond
      Quand l'utilisateur lance l'import des connexions DBeaver
      Alors le résumé ignore la connexion "legacy-mysql" avec la raison "only PostgreSQL"
      Et le résumé ignore la connexion "jumped" avec la raison "jump servers"

    Scénario: Sans connexion ignorée ni bloc proposé le résumé n'a pas de section vide
      Étant donné un espace de travail DBeaver avec une connexion à mot de passe natif enregistré
      Quand l'utilisateur lance l'import des connexions DBeaver
      Alors le résumé ne contient pas "Skipped:"
      Et le résumé ne propose aucun bloc Host

  Règle: Les doublons de nom sont importés sous les noms suffixés que montre list_connections

    Scénario: Deux connexions DBeaver du même nom sont importées sous leurs noms suffixés
      Étant donné un espace de travail DBeaver avec deux connexions nommées "app"
      Quand l'utilisateur lance l'import des connexions DBeaver
      Et l'agent liste les connexions
      Alors chaque nom de connexion est listé une seule fois avec la source "file"
      Et le fichier de connexions ne déclare aucune connexion nommée exactement "app"
