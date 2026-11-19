# language: fr
@connections
Fonctionnalité: Découverte des connexions
  L'agent ne connaît que les connexions déjà configurées dans DBeaver et n'en voit jamais les identifiants.

  Scénario: Une connexion dont le mot de passe est enregistré est listée comme utilisable
    Étant donné un espace de travail DBeaver où la connexion "canary-local" a son mot de passe enregistré
    Quand l'agent liste les connexions
    Alors la connexion "canary-local" est listée comme disponible
    Et la liste ne montre ni hôte, ni utilisateur, ni mot de passe

  Scénario: Une connexion sans mot de passe enregistré est listée avec la raison
    Étant donné un espace de travail DBeaver où la connexion "canary-local" n'a pas de mot de passe enregistré
    Quand l'agent liste les connexions
    Alors la connexion "canary-local" est listée comme indisponible
    Et la raison indique qu'aucun mot de passe n'est enregistré dans DBeaver
