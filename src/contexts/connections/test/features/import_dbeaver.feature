# language: fr
@connections
Fonctionnalité: Import des connexions DBeaver
  Un utilisateur de DBeaver génère une fois son fichier de connexions, sans qu'aucun secret ne soit écrit, et sait ce qu'il lui reste à faire.

  Scénario: L'import écrit une connexion par connexion PostgreSQL sans le mot de passe enregistré dans DBeaver
    Étant donné un espace de travail DBeaver avec une connexion à mot de passe natif enregistré
    Quand l'utilisateur lance l'import des connexions DBeaver
    Alors le fichier de connexions déclare la connexion "canary-local"
    Et le fichier de connexions ne contient pas le mot de passe enregistré dans DBeaver

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

  Scénario: Une connexion native devient pgpass et le résumé demande une entrée pgpass ou une commande
    Étant donné un espace de travail DBeaver avec une connexion à mot de passe natif enregistré
    Quand l'utilisateur lance l'import des connexions DBeaver
    Alors la connexion "canary-local" utilise l'authentification "pgpass"
    Et le résumé demande une entrée pgpass ou une commande de mot de passe

  Scénario: Une connexion pgpass sur un hôte Azure devient azure
    Étant donné un espace de travail DBeaver avec une connexion pgpass sur un serveur Azure
    Quand l'utilisateur lance l'import des connexions DBeaver
    Alors la connexion "azure-shop" utilise l'authentification "azure"

  Scénario: Une connexion à tunnel SSH reçoit un alias et le résumé propose un bloc Host
    Étant donné un espace de travail DBeaver avec une connexion qui traverse un bastion SSH
    Quand l'utilisateur lance l'import des connexions DBeaver
    Alors la connexion "canary-local" porte l'alias SSH "bastion.corp.example"
    Et le résumé propose un bloc Host avec l'hôte, le port, l'utilisateur et le fichier de clé

  Scénario: Les connexions MySQL et à serveur de rebond sont ignorées avec leur raison
    Étant donné un espace de travail DBeaver avec une connexion MySQL et une connexion à serveur de rebond
    Quand l'utilisateur lance l'import des connexions DBeaver
    Alors le résumé ignore la connexion "legacy-mysql" avec la raison "only PostgreSQL"
    Et le résumé ignore la connexion "jumped" avec la raison "jump servers"

  Scénario: Deux connexions DBeaver du même nom sont importées sous leurs noms suffixés
    Étant donné un espace de travail DBeaver avec deux connexions nommées "app"
    Quand l'utilisateur lance l'import des connexions DBeaver
    Et l'agent liste les connexions
    Alors chaque nom de connexion est listé une seule fois avec la source "file"
    Et le fichier de connexions ne déclare aucune connexion nommée exactement "app"
