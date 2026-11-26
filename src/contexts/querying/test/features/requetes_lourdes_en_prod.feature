# language: fr
@integration @querying
Fonctionnalité: Requêtes lourdes sur une connexion de production
  Sur une connexion de type prod, query, find et aggregate estiment la requête avant de l'exécuter, sans jamais l'exécuter pour l'estimer.
  Une requête estimée lourde est refusée avec le code heavy_query_requires_confirmation et son estimation :
  l'assistant la montre au développeur, et ne la relance avec confirm_heavy qu'après son accord.
  Les seuils par défaut (coût 100000, 1000000 lignes parcourues) se règlent par connexion avec heavy_cost_limit et heavy_scan_limit.

  Règle: PostgreSQL : un plan au-dessus du seuil de coût est refusé avant d'atteindre la base

    Contexte:
      Étant donné l'agent connecté à la base de test déclarée prod avec « heavy_cost_limit = 100 »

    Scénario: Un filtre sans index sur une grande table est refusé avec son estimation
      Quand l'agent exécute la requête "SELECT id FROM big_numbers WHERE id = 7"
      Alors l'outil est refusé avec le code "heavy_query_requires_confirmation"
      Et l'estimation donne un coût au-dessus de la limite de 100
      Et l'indice de l'erreur contient "confirm_heavy: true"
      Et l'erreur ne peut pas être retentée

    Scénario: Après l'accord du développeur, la même requête est exécutée
      Quand l'agent exécute avec confirm_heavy la requête "SELECT id FROM big_numbers WHERE id = 7"
      Alors la requête réussit
      Et la première cellule de la réponse vaut 7

    Scénario: Une limite qui arrête le parcours rend la requête légère
      Quand l'agent exécute la requête "SELECT id FROM big_numbers LIMIT 1"
      Alors la requête réussit

    Scénario: L'estimation ne cite jamais la valeur réelle d'un filtre
      Quand l'agent exécute la requête "SELECT id FROM customers, big_numbers WHERE customers.email = 'ylwenna.kervadec@canary-mail.fr'"
      Alors l'outil est refusé avec le code "heavy_query_requires_confirmation"
      Et la réponse ne contient aucune valeur canari

  Règle: MongoDB : un parcours complet d'une collection au-dessus du seuil est refusé

    Contexte:
      Étant donné la connexion MongoDB de test déclarée prod avec « heavy_scan_limit = 1 »

    Scénario: Un filtre sans index parcourt toute la collection et est refusé
      Quand l'agent cherche dans "orders" avec le filtre {"status": "paid"}
      Alors l'outil est refusé avec le code "heavy_query_requires_confirmation"
      Et l'estimation nomme le parcours "COLLSCAN" de "orders"
      Et la réponse ne contient aucune valeur canari

    Scénario: Après l'accord du développeur, la même recherche est exécutée
      Quand l'agent cherche avec confirm_heavy dans "orders" avec le filtre {"status": "paid"}
      Alors la réponse compte 1 documents

    Scénario: Une lecture sans filtre arrêtée par sa limite reste légère
      Quand l'agent cherche dans "orders" tous les documents
      Alors la réponse compte 3 documents

    Scénario: Une agrégation qui groupe toute la collection est refusée
      Quand l'agent agrège "orders" avec [{"$group": {"_id": "$status", "n": {"$sum": 1}}}]
      Alors l'outil est refusé avec le code "heavy_query_requires_confirmation"
      Et l'estimation nomme le parcours "COLLSCAN" de "orders"

    Scénario: Une lecture par index n'est pas estimée lourde
      Quand l'agent cherche dans "customers" avec le filtre {"contact.email": "nobody"}
      Alors la réponse compte 0 documents
