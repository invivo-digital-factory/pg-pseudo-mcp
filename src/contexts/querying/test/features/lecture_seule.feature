# language: fr
@querying
Fonctionnalité: Lecture seule
  L'agent lit PostgreSQL avec query et explain, MongoDB avec find et aggregate, sans jamais pouvoir modifier une base ni agir sur son serveur.
  Chaque demande passe d'abord par une garde qui n'accepte que la lecture : un refus tombe avant tout contact avec la base,
  nomme la règle qui l'arrête et ne reprend aucune valeur de la demande.

  Règle: PostgreSQL : une seule instruction de lecture est acceptée par appel
    Sont acceptés SELECT, WITH ... SELECT, VALUES et EXPLAIN sans ANALYZE ; tout le reste est refusé avec sa règle.

    Plan du Scénario: Une lecture passe la garde
      Quand la garde SQL examine la requête "<requête>"
      Alors la demande est acceptée
      Et 0 valeur a été traduite

      Exemples:
        | requête                                                                                                     |
        | SELECT id, email FROM customers WHERE id = 1                                                                |
        | WITH recent AS (SELECT * FROM orders WHERE placed_at > now() - interval '7 days') SELECT count(*) FROM recent |
        | VALUES (1, 'a')                                                                                             |
        | EXPLAIN SELECT * FROM orders WHERE customer_id = 1                                                          |
        | EXPLAIN (ANALYZE false) SELECT * FROM orders                                                                |
        | SELECT status::text, row_number() OVER (PARTITION BY customer_id ORDER BY placed_at) FROM orders            |
        | SELECT * FROM generate_series(1, 3) g                                                                       |
        | SELECT * FROM orders o, LATERAL jsonb_array_elements(o.items) e                                             |
        | SELECT * FROM unnest(ARRAY[1, 2]) u                                                                         |

    Plan du Scénario: Une écriture, un changement de session ou plusieurs instructions sont refusés avec leur règle
      Étant donné une connexion PostgreSQL vers une base injoignable
      Quand l'agent exécute la requête "<requête>"
      Alors la requête est refusée par la règle "<règle>"

      Exemples:
        | requête                                                                 | règle                 |
        | DELETE FROM orders                                                      | statement_not_allowed |
        | UPDATE orders SET amount = 0                                            | statement_not_allowed |
        | INSERT INTO orders (id) VALUES (1)                                      | statement_not_allowed |
        | CREATE TABLE t (id int)                                                 | statement_not_allowed |
        | DROP TABLE orders                                                       | statement_not_allowed |
        | TRUNCATE orders                                                         | statement_not_allowed |
        | COPY orders TO '/tmp/out.csv'                                           | statement_not_allowed |
        | SET transaction_read_only = off                                         | statement_not_allowed |
        | RESET ALL                                                               | statement_not_allowed |
        | BEGIN                                                                   | statement_not_allowed |
        | WITH d AS (DELETE FROM orders RETURNING *) SELECT * FROM d              | writable_cte          |
        | WITH x AS (UPDATE customers SET nom = 'x' RETURNING id) SELECT * FROM x | writable_cte          |
        | SELECT * INTO t FROM orders                                             | select_into           |
        | SELECT * FROM orders FOR UPDATE                                         | locking_clause        |
        | SELECT 1; DELETE FROM orders                                            | multi_statement       |
        | EXPLAIN ANALYZE SELECT * FROM orders                                    | explain_analyze       |
        | EXPLAIN (FORMAT JSON, ANALYSE on) SELECT * FROM orders                  | explain_analyze       |
        | SELEC id FROM                                                           | parse_error           |

    @integration
    Scénario: Un DELETE refusé laisse la table intacte
      Étant donné l'agent connecté à la base de test
      Quand l'agent exécute la requête "SELECT id FROM orders ORDER BY id"
      Alors la requête réussit
      Et la réponse compte 4 lignes
      Quand l'agent exécute la requête "DELETE FROM orders"
      Alors la requête est refusée parce qu'elle n'est pas en lecture seule
      Quand l'agent exécute la requête "SELECT count(*) FROM orders"
      Alors la requête réussit
      Et la première cellule de la réponse vaut 4

  Règle: PostgreSQL : une fonction qui agit sur le serveur est refusée en la nommant, où qu'elle soit appelée
    Session, signaux, verrous, fichiers, objets larges, connexions distantes, sauvegarde, maintenance et requêtes en texte sont refusés,
    dans la projection comme dans FROM.

    Plan du Scénario: La fonction <fonction> est refusée
      Étant donné une connexion PostgreSQL vers une base injoignable
      Quand l'agent exécute la requête "<requête>"
      Alors la requête est refusée par la règle "function_not_allowed"
      Et le refus nomme "<fonction>"

      Exemples:
        | requête                                                         | fonction             |
        | SELECT set_config('transaction_read_only', 'off', false)        | set_config           |
        | SELECT current_setting('my.secret')                             | current_setting      |
        | SELECT pg_catalog.pg_terminate_backend(1)                       | pg_terminate_backend |
        | SELECT * FROM LATERAL pg_terminate_backend(1) t                 | pg_terminate_backend |
        | SELECT * FROM unnest(ARRAY[pg_cancel_backend(1)]) u             | pg_cancel_backend    |
        | SELECT * FROM orders o, LATERAL pg_advisory_lock(4242) l        | pg_advisory_lock     |
        | SELECT nextval('audit_seq')                                     | nextval              |
        | SELECT pg_sleep(10)                                             | pg_sleep             |
        | SELECT pg_reload_conf()                                         | pg_reload_conf       |
        | SELECT pg_stat_reset()                                          | pg_stat_reset        |
        | SELECT pg_switch_wal()                                          | pg_switch_wal        |
        | SELECT lo_import('/etc/passwd')                                 | lo_import            |
        | SELECT * FROM LATERAL pg_read_file('/etc/passwd') f             | pg_read_file         |
        | SELECT * FROM dblink('host=x', 'select 1') AS t(a int)          | dblink               |
        | SELECT * FROM crosstab('select 1, 1, 1') AS t(a int, b int)     | crosstab             |
        | SELECT * FROM crosstab3('select 1, 1, 1')                       | crosstab3            |
        | SELECT * FROM connectby('t', 'id', 'parent', '1', 0) AS t(id text, parent text, level int) | connectby |
        | SELECT query_to_xml('select 1', true, true, '')                 | query_to_xml         |
        | SELECT pg_backup_start('x')                                     | pg_backup_start      |
        | SELECT gin_clean_pending_list('orders_gin')                     | gin_clean_pending_list |
        | SELECT pg_background_launch('DELETE FROM orders')               | pg_background_launch |

  Règle: PostgreSQL : seul le catalogue structurel est lisible
    Les statistiques, l'activité, les réglages et les correspondances d'utilisateurs exposent des données ou des secrets :
    leur lecture est refusée et le refus renvoie vers les outils de schéma.

    Plan du Scénario: Une vue du serveur est refusée avec un renvoi vers describe_table
      Étant donné une connexion PostgreSQL vers une base injoignable
      Quand l'agent exécute la requête "<requête>"
      Alors la requête est refusée par la règle "<règle>"
      Et le refus renvoie vers "describe_table"

      Exemples:
        | requête                                         | règle                 |
        | SELECT * FROM pg_stats                          | relation_not_allowed  |
        | SELECT query FROM pg_catalog.pg_stat_activity   | relation_not_allowed  |
        | SELECT * FROM pg_settings                       | relation_not_allowed  |
        | SELECT * FROM pg_user_mappings                  | relation_not_allowed  |
        | SELECT query FROM public.pg_stat_statements     | relation_not_allowed  |
        | SELECT datname FROM pg_database                 | relation_not_allowed  |
        | SELECT * FROM pg_stat_get_activity(NULL)        | function_not_allowed  |

    Plan du Scénario: Une vue d'information_schema qui n'est pas structurelle est refusée
      Étant donné une connexion PostgreSQL vers une base injoignable
      Quand l'agent exécute la requête "SELECT * FROM information_schema.<vue>"
      Alors la requête est refusée par la règle "relation_not_allowed"

      Exemples:
        | vue                     |
        | check_constraints       |
        | user_mapping_options    |
        | foreign_server_options  |

    Plan du Scénario: Le catalogue structurel reste lisible
      Quand la garde SQL examine la requête "<requête>"
      Alors la demande est acceptée

      Exemples:
        | requête                                                                       |
        | SELECT relname FROM pg_catalog.pg_class                                       |
        | SELECT table_name FROM information_schema.tables                              |
        | SELECT column_name FROM information_schema.columns WHERE table_name = 'orders' |

  Règle: MongoDB : seules les étapes d'agrégation de lecture sont acceptées
    La liste autorisée est publiée par server_info ; une étape absente de la liste est refusée avec son nom et sa position,
    même cachée dans un sous-pipeline, avant qu'un pseudonyme soit cherché.

    Contexte:
      Étant donné l'agent connecté à une base MongoDB

    Plan du Scénario: Une agrégation de lecture est acceptée
      Quand l'agent lance l'agrégation suivante :
        """
        <pipeline>
        """
      Alors la demande est acceptée

      Exemples:
        | pipeline                                                                                                                      |
        | [{"$match": {"status": "PAID"}}, {"$group": {"_id": "$status", "n": {"$sum": 1}}}, {"$sort": {"n": -1}}]                     |
        | [{"$lookup": {"from": "customers", "localField": "customerId", "foreignField": "_id", "as": "buyer"}}, {"$unwind": "$buyer"}] |
        | [{"$facet": {"byStatus": [{"$sortByCount": "$status"}], "total": [{"$count": "n"}]}}]                                        |
        | [{"$bucketAuto": {"groupBy": "$amount", "buckets": 4}}]                                                                       |
        | [{"$geoNear": {"near": {"type": "Point", "coordinates": [2.35, 48.85]}, "distanceField": "d"}}, {"$limit": 5}]               |
        | [{"$documents": [{"a": 1}]}]                                                                                                  |
        | [{"$sample": {"size": 3}}, {"$skip": 1}, {"$limit": 2}]                                                                       |

    Plan du Scénario: L'étape <étape> est refusée
      Quand l'agent lance l'agrégation suivante :
        """
        [{"<étape>": {}}]
        """
      Alors la demande est refusée par la règle "mongo_stage_not_allowed"
      Et le refus nomme "<étape>"
      Et rien n'a été demandé au magasin de pseudonymes

      Exemples:
        | étape              |
        | $merge             |
        | $currentOp         |
        | $listSessions      |
        | $collStats         |
        | $indexStats        |
        | $planCacheStats    |
        | $changeStream      |
        | $searchMeta        |
        | $vectorSearch      |
        | $listSearchIndexes |
        | $querySettings     |
        | $mapReduce         |
        | $fooBar            |

    Scénario: Un $out en deuxième étape est refusé en nommant l'étape et sa position
      Quand l'agent lance l'agrégation suivante :
        """
        [{"$match": {}}, {"$out": "copy"}]
        """
      Alors la demande est refusée par la règle "mongo_stage_not_allowed"
      Et le refus nomme "$out"
      Et le refus nomme "stage 2"
      Et rien n'a été demandé au magasin de pseudonymes

    Plan du Scénario: Une étape d'écriture cachée dans un sous-pipeline est refusée
      Quand l'agent lance l'agrégation suivante :
        """
        <pipeline>
        """
      Alors la demande est refusée par la règle "mongo_stage_not_allowed"
      Et le refus nomme "<étape>"

      Exemples:
        | pipeline                                                                                | étape  |
        | [{"$facet": {"copie": [{"$match": {}}, {"$merge": {"into": "copy"}}]}}]                 | $merge |
        | [{"$lookup": {"from": "c", "pipeline": [{"$merge": {"into": "x"}}], "as": "r"}}]        | $merge |
        | [{"$unionWith": {"coll": "c", "pipeline": [{"$out": "x"}]}}]                            | $out   |

    Scénario: $geoNear n'est accepté qu'en première étape
      Quand l'agent lance l'agrégation suivante :
        """
        [{"$match": {}}, {"$geoNear": {"near": [0, 0], "distanceField": "d"}}]
        """
      Alors la demande est refusée par la règle "mongo_stage_position"

  Règle: MongoDB : le JavaScript côté serveur et les opérateurs inconnus sont refusés
    $where, $function, $accumulator et les types BSON exécutables sont refusés partout : filtre, projection, $expr, $let, accumulateurs.

    Contexte:
      Étant donné l'agent connecté à une base MongoDB

    Plan du Scénario: Le filtre <nom> est refusé en nommant l'opérateur
      Quand l'agent cherche avec le filtre suivant :
        """
        <filtre>
        """
      Alors la demande est refusée par la règle "<règle>"
      Et le refus nomme "<nom>"
      Et rien n'a été demandé au magasin de pseudonymes

      Exemples:
        | filtre                                                                        | règle                           | nom           |
        | {"$where": "this.a == 1"}                                                     | mongo_operator_not_allowed      | $where        |
        | {"$expr": {"$function": {"body": "function() {}", "args": [], "lang": "js"}}} | mongo_operator_not_allowed      | $function     |
        | {"items": {"$elemMatch": {"qty": 1, "$where": "true"}}}                       | mongo_operator_not_allowed      | $where        |
        | {"a": {"$code": "function() {}"}}                                             | mongo_extended_json_not_allowed | $code         |
        | {"a": {"$eq": {"$codeWithScope": {"$code": "f", "$scope": {}}}}}              | mongo_operator_not_allowed      | $codeWithScope |
        | {"$jsonSchema": {}}                                                           | mongo_operator_not_allowed      | $jsonSchema   |
        | {"status": {"$fooBar": 1}}                                                    | mongo_operator_not_allowed      | $fooBar       |

    Plan du Scénario: Le JavaScript dans une étape d'agrégation est refusé
      Quand l'agent lance l'agrégation suivante :
        """
        <pipeline>
        """
      Alors la demande est refusée par la règle "mongo_operator_not_allowed"
      Et le refus nomme "<nom>"

      Exemples:
        | pipeline                                                                                                      | nom          |
        | [{"$group": {"_id": null, "x": {"$accumulator": {"init": "function(){}", "lang": "js"}}}}]                    | $accumulator |
        | [{"$project": {"x": {"$let": {"vars": {"v": {"$function": {"body": "f", "args": [], "lang": "js"}}}, "in": "$$v"}}}}] | $function |
        | [{"$match": {}}, {"$project": {"x": {"$fooBar": "$a"}}}]                                                      | stage 2      |

  Règle: MongoDB : un argument que le serveur recopie dans ses erreurs doit être une constante
    Le type cible de $convert, l'unité, le fuseau et le format des dates, le motif et les options des expressions régulières
    ne peuvent pas lire un champ : sinon le message d'erreur du serveur citerait une valeur réelle.

    Contexte:
      Étant donné l'agent connecté à une base MongoDB

    Plan du Scénario: Un argument lu dans un champ est refusé sans nommer le champ
      Quand l'agent lance l'agrégation suivante :
        """
        [{"$project": {"n": <expression>}}]
        """
      Alors la demande est refusée par la règle "mongo_argument_not_constant"
      Et le refus ne contient pas "lastName"

      Exemples:
        | expression                                                                   |
        | {"$convert": {"input": 1, "to": "$lastName"}}                                |
        | {"$dateTrunc": {"date": "$birthDate", "unit": "$lastName"}}                  |
        | {"$dateToString": {"date": "$birthDate", "timezone": "$lastName"}}           |
        | {"$hour": {"date": "$birthDate", "timezone": "$lastName"}}                   |
        | {"$regexFind": {"input": "x", "regex": "$lastName"}}                         |
        | {"$regexMatch": {"input": "x", "regex": "a", "options": "$lastName"}}       |
        | {"$dateToString": {"date": "$birthDate", "format": "$lastName"}}           |

    Plan du Scénario: Le même argument écrit en constante est accepté
      Quand l'agent lance l'agrégation suivante :
        """
        [{"$project": {"n": <expression>}}]
        """
      Alors la demande est acceptée

      Exemples:
        | expression                                                                            |
        | {"$convert": {"input": "$amount", "to": "int", "onError": 0}}                         |
        | {"$dateTrunc": {"date": "$birthDate", "unit": "week", "startOfWeek": "monday"}}       |
        | {"$dateToString": {"date": "$birthDate", "format": "%Y", "timezone": "Europe/Paris"}} |
        | {"$regexMatch": {"input": "$status", "regex": "^p", "options": "i"}}                  |

  Règle: MongoDB : seules les collections d'application de la base demandée sont lisibles
    Les bases admin, local et config et les collections system.* sont refusées ; une jointure ou une union ne nomme qu'une collection de la même base.

    Contexte:
      Étant donné l'agent connecté à une base MongoDB

    Plan du Scénario: La base <base> est refusée
      Quand l'agent cherche dans la base "<base>"
      Alors la demande est refusée par la règle "mongo_system_namespace"

      Exemples:
        | base   |
        | admin  |
        | local  |
        | config |
        | Admin  |

    Plan du Scénario: Un nom de collection système ou invalide est refusé
      Quand l'agent cherche dans la collection "<collection>"
      Alors la demande est refusée par la règle "<règle>"

      Exemples:
        | collection   | règle                  |
        | system.users | mongo_system_namespace |
        | $cmd         | mongo_invalid_name     |
        |              | mongo_invalid_name     |

    Plan du Scénario: Une jointure vers une collection système ou une autre base est refusée
      Quand l'agent lance l'agrégation suivante :
        """
        <pipeline>
        """
      Alors la demande est refusée par la règle "<règle>"

      Exemples:
        | pipeline                                                                                                           | règle                  |
        | [{"$unionWith": "system.users"}]                                                                                   | mongo_system_namespace |
        | [{"$lookup": {"from": "system.js", "pipeline": [], "as": "r"}}]                                                    | mongo_system_namespace |
        | [{"$lookup": {"from": {"db": "admin", "coll": "system.users"}, "localField": "a", "foreignField": "b", "as": "c"}}] | mongo_cross_database   |
        | [{"$unionWith": {"db": "other", "coll": "c"}}]                                                                     | mongo_cross_database   |

    Scénario: Le refus d'une jointure vers une autre base nomme l'argument fautif
      Quand l'agent lance l'agrégation suivante :
        """
        [{"$lookup": {"from": {"db": "admin", "coll": "system.users"}, "localField": "a", "foreignField": "b", "as": "c"}}]
        """
      Alors la demande est refusée par la règle "mongo_cross_database"
      Et le refus nomme "from"

  Règle: MongoDB : la taille d'une demande est bornée
    Au plus 50 étapes en comptant les sous-pipelines, 32 niveaux d'imbrication, 1 000 éléments par tableau et 64 Ko de JSON.

    Contexte:
      Étant donné l'agent connecté à une base MongoDB

    Scénario: 50 étapes passent, 51 sont refusées
      Quand l'agent lance une agrégation de 50 étapes $match
      Alors la demande est acceptée
      Quand l'agent lance une agrégation de 51 étapes $match
      Alors la demande est refusée par la règle "mongo_pipeline_too_long"
      Et le refus nomme "50"

    Scénario: Les étapes des branches d'un $facet comptent dans le plafond
      Quand l'agent lance un $facet de 2 branches de 24 étapes $match
      Alors la demande est acceptée
      Quand l'agent lance un $facet de 3 branches de 20 étapes $match
      Alors la demande est refusée par la règle "mongo_pipeline_too_long"

    Scénario: 32 niveaux d'imbrication passent, 33 sont refusés
      Quand l'agent cherche avec un filtre imbriqué sur 32 niveaux
      Alors la demande est acceptée
      Quand l'agent cherche avec un filtre imbriqué sur 33 niveaux
      Alors la demande est refusée par la règle "mongo_nesting_too_deep"
      Et le refus nomme "32"

    Scénario: Un tableau de 1000 valeurs passe, 1001 sont refusées
      Quand l'agent cherche avec un $in de 1000 valeurs
      Alors la demande est acceptée
      Quand l'agent cherche avec un $in de 1001 valeurs
      Alors la demande est refusée par la règle "mongo_array_too_long"
      Et le refus nomme "1000"

    Scénario: Une demande de plus de 64 Ko est refusée sans recopier son contenu
      Quand l'agent cherche une valeur texte de 70 Ko
      Alors la demande est refusée par la règle "mongo_request_too_large"
      Et le refus nomme "65536"
      Et le refus ne contient pas "xxxx"

  Règle: Un refus ne reprend aucune valeur de la demande

    Scénario: Un refus PostgreSQL ne cite pas la valeur filtrée
      Étant donné une connexion PostgreSQL vers une base injoignable
      Quand l'agent exécute la requête "SELECT pg_sleep(1) FROM customers WHERE email = 'ylwenna.kervadec@canary-mail.fr'"
      Alors la requête est refusée par la règle "function_not_allowed"
      Et le refus ne contient aucune valeur canari

    Scénario: Un refus MongoDB ne cite pas la valeur filtrée
      Étant donné l'agent connecté à une base MongoDB
      Quand l'agent cherche avec le filtre suivant :
        """
        {"email": "ylwenna.kervadec@canary-mail.fr", "$where": "this.email == 'ylwenna.kervadec@canary-mail.fr'"}
        """
      Alors la demande est refusée par la règle "mongo_operator_not_allowed"
      Et le refus ne contient aucune valeur canari

  @integration
  Règle: MongoDB : la garde protège même un utilisateur autorisé à écrire
    Avec un utilisateur readWrite, la garde est la seule protection contre l'écriture.

    Scénario: Un $merge lancé par un utilisateur readWrite est refusé et n'écrit rien
      Étant donné la connexion MongoDB de test avec un utilisateur autorisé à écrire
      Quand l'agent agrège "customers" avec [{"$match": {}}, {"$merge": {"into": "stolen"}}]
      Alors la requête est refusée par la règle "mongo_stage_not_allowed"
      Et le refus nomme "$merge"
      Quand l'agent cherche dans "stolen" tous les documents
      Alors la réponse compte 0 documents
