# language: fr
@querying
Fonctionnalité: Suivi d'un client par son pseudonyme dans MongoDB
  Un pseudonyme réutilisé dans un filtre est remplacé par la valeur réelle là où il est comparé à un champ, et laissé tel quel ailleurs avec un avertissement.

  Contexte:
    Étant donné l'agent connecté à une base MongoDB
    Et le pseudonyme "noemie.vasseur@example.net" désigne la valeur réelle "ylwenna.kervadec@canary-mail.fr"

  Plan du Scénario: AE6, un pseudonyme comparé à un champ est traduit avant l'envoi
    Quand l'agent cherche avec le filtre suivant :
      """
      <filtre>
      """
    Alors la demande est acceptée
    Et 1 valeur a été traduite
    Et le filtre envoyé ne contient plus le pseudonyme

    Exemples:
      | filtre                                                                  |
      | {"customer.email": "noemie.vasseur@example.net"}                        |
      | {"customer.email": {"$in": ["noemie.vasseur@example.net"]}}             |
      | {"customer.email": {"$nin": ["noemie.vasseur@example.net"]}}            |
      | {"contacts": {"$elemMatch": {"email": "noemie.vasseur@example.net"}}}   |
      | {"$expr": {"$eq": ["$contact.email", "noemie.vasseur@example.net"]}}    |

  Scénario: Un pseudonyme dans le $match d'un sous-pipeline est traduit
    Quand l'agent lance l'agrégation suivante :
      """
      [{"$lookup": {"from": "orders", "pipeline": [{"$match": {"customer.email": "noemie.vasseur@example.net"}}], "as": "commandes"}}]
      """
    Alors la demande est acceptée
    Et 1 valeur a été traduite

  Plan du Scénario: AE7, un pseudonyme hors comparaison reste tel quel avec un avertissement
    Quand l'agent cherche avec le filtre suivant :
      """
      <filtre>
      """
    Alors la demande est acceptée
    Et 0 valeur a été traduite
    Et la demande porte l'avertissement "pseudonym_not_translated"

    Exemples:
      | filtre                                                                          |
      | {"contact.email": {"$regex": "^noemie.vasseur@example.net"}}                    |
      | {"contact.email": {"$gt": "noemie.vasseur@example.net"}}                        |
      | {"$expr": {"$eq": [{"$concat": ["$a", "noemie.vasseur@example.net"]}, "x"]}}    |

  Scénario: Un pseudonyme inconnu sur ce poste est comparé tel quel
    Quand l'agent cherche avec le filtre suivant :
      """
      {"email": "inconnu.total@example.org"}
      """
    Alors la demande est acceptée
    Et 0 valeur a été traduite
    Et la demande porte l'avertissement "unresolved_pseudonym"

  Scénario: R18, un pseudonyme numérique redevient un nombre quand la valeur réelle est en chiffres
    Étant donné le pseudonyme "734918265" désigne la valeur réelle "418273652"
    Quand l'agent cherche avec le filtre suivant :
      """
      {"phone": 734918265}
      """
    Alors la demande est acceptée
    Et le filtre envoyé vaut :
      """
      {"phone": 418273652}
      """
