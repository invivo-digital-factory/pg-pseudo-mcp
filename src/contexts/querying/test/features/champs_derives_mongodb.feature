# language: fr
@querying
Fonctionnalité: Champs dérivés d'une agrégation MongoDB
  Un champ de sortie qui recopie ou renomme un champ garde la classification de ce champ dans sa collection d'origine.
  Un champ calculé n'est en clair que si chaque champ lu est en clair par politique et chaque opérateur préserve le clair ; sinon il est dérivé et teinté.
  Ce que la lignée ne suit pas est inconnu, donc teinté.

  Contexte:
    Étant donné la politique de colonnes partagée suivante :
      """
      [mongodb.clear]
      "shop.customers" = ["status", "amount"]
      """
    Quand la politique de colonnes est chargée

  Scénario: Une projection renomme, recopie et calcule
    Quand l'agrégation suivante part sur "shop.customers" :
      """
      [{"$project": {"who": {"$concat": ["$firstName", " ", "$lastName"]}, "mail": "$contact.email", "status": 1}}]
      """
    Alors le champ de sortie "who" est dérivé et teinté
    Et le champ de sortie "mail" vient du champ "contact.email" de "shop.customers"
    Et le champ de sortie "status" vient du champ "status" de "shop.customers"
    Et le champ "status" de "shop.customers" est en clair

  Scénario: Une clé de regroupement garde sa source et une somme n'est en clair que si le champ sommé l'est par politique
    Quand l'agrégation suivante part sur "shop.customers" :
      """
      [{"$group": {"_id": "$status", "total": {"$sum": "$amount"}, "oldest": {"$max": "$birthDate"}}}]
      """
    Alors le champ de sortie "_id" vient du champ "status" de "shop.customers"
    Et le champ de sortie "total" est dérivé et prouvé en clair
    Et le champ de sortie "oldest" est dérivé et teinté

  Scénario: Une jointure renvoie aux champs de la collection jointe
    Quand l'agrégation suivante part sur "shop.orders" :
      """
      [{"$lookup": {"from": "customers", "localField": "customerId", "foreignField": "_id", "as": "buyer"}}]
      """
    Alors le champ de sortie "buyer.email" vient du champ "email" de "shop.customers"
    Et le champ de sortie "total" vient du champ "total" de "shop.orders"

  Scénario: Une nouvelle racine prise sur un champ ré-enracine les sources
    Quand l'agrégation suivante part sur "shop.customers" :
      """
      [{"$replaceRoot": {"newRoot": "$contact"}}]
      """
    Alors le champ de sortie "email" vient du champ "contact.email" de "shop.customers"

  Scénario: Une mise en majuscules d'un champ en clair reste en clair, une sous-chaîne d'un champ personnel est teintée
    Quand l'agrégation suivante part sur "shop.customers" :
      """
      [{"$project": {"loud": {"$toUpper": "$status"}, "start": {"$substrCP": ["$email", 0, 3]}}}]
      """
    Alors le champ de sortie "loud" est dérivé et prouvé en clair
    Et le champ de sortie "start" est dérivé et teinté

  Plan du Scénario: Ce que la lignée ne suit pas est inconnu et teinté
    Quand l'agrégation <pipeline> part sur "shop.customers"
    Alors le champ de sortie "<champ>" est de lignée inconnue
    Et le champ de sortie "<champ>" est dérivé et teinté

    Exemples:
      | pipeline                                                                      | champ           |
      | [{"$project": {"whole": "$$ROOT"}}]                                           | whole.email     |
      | [{"$replaceRoot": {"newRoot": {"$mergeObjects": ["$contact", "$address"]}}}] | email           |
      | [{"$project": {"pairs": {"$objectToArray": "$contact"}}}]                     | pairs           |
      | [{"$facet": {"byStatus": [{"$project": {"status": 1}}]}}]                     | byStatus.status |
      | [{"$unionWith": {"coll": "prospects"}}]                                       | email           |
