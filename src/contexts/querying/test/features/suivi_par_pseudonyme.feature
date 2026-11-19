# language: fr
@integration @querying
Fonctionnalité: Suivi d'un client par son pseudonyme
  L'agent retrouve les données d'un client sans jamais connaître son adresse réelle.

  Scénario: Les commandes d'un client se retrouvent à partir de son adresse pseudonymisée
    Étant donné l'agent connecté à la base de test
    Quand l'agent exécute la requête "SELECT email FROM customers WHERE id = 1"
    Alors la première cellule de la réponse est un pseudonyme
    Quand l'agent cherche les commandes du client dont l'adresse est ce pseudonyme
    Alors la requête réussit
    Et la réponse liste les commandes 10 et 11
    Et le pseudonyme a été traduit en une valeur réelle côté serveur
