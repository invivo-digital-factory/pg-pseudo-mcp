# language: fr
@integration @querying
Fonctionnalité: Plan d'exécution
  explain montre le plan d'une requête PostgreSQL sans l'exécuter, pour vérifier un index ou un coût.
  La requête passe par la même garde que query, ses pseudonymes sont traduits de la même façon,
  et les valeurs réelles que le plan recopie y reviennent pseudonymisées.

  Règle: Le plan cite le pseudonyme filtré, jamais la valeur réelle

    Scénario: Le plan d'un filtre sur un pseudonyme montre le pseudonyme
      Étant donné l'agent connecté à la base de test
      Quand l'agent exécute la requête "SELECT email FROM customers WHERE id = 1"
      Alors la première cellule de la réponse est un pseudonyme
      Quand l'agent demande le plan du filtre des clients sur ce pseudonyme
      Alors le plan est rendu
      Et le pseudonyme a été traduit en une valeur réelle côté serveur
      Et la réponse cite le pseudonyme
      Et la réponse ne contient aucune valeur canari

    Plan du Scénario: Le plan se demande au format <format>
      Étant donné l'agent connecté à la base de test
      Quand l'agent demande le plan de "EXPLAIN (FORMAT <format>) SELECT * FROM customers WHERE email = 'ylwenna.kervadec@canary-mail.fr'"
      Alors le plan est rendu
      Et la réponse ne contient aucune valeur canari

      Exemples:
        | format |
        | JSON   |
        | YAML   |

  Règle: Le plan ne s'obtient jamais en exécutant la requête

    Scénario: explain refuse ANALYZE
      Étant donné l'agent connecté à la base de test
      Quand l'agent demande le plan de "EXPLAIN ANALYZE SELECT * FROM orders"
      Alors la requête est refusée par la règle "explain_analyze"
