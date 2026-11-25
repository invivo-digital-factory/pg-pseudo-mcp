# language: fr
@querying
Fonctionnalité: Limites des réponses
  Une réponse reste assez petite pour le contexte de l'agent : nombre de lignes ou de documents, taille en octets, longueur d'une valeur
  et durée d'exécution sont bornés. Par défaut 50 lignes, 16 Ko et 10 s ; au plus 500 lignes, 64 Ko et 60 s ; une valeur texte est coupée à 500 caractères.
  Une réponse coupée le dit dans truncated et indique comment lire la suite : pagination par clé, ou cell_offset pour le reste d'une valeur longue.

  @integration
  Règle: Une limite demandée hors de ses bornes est ramenée dans ses bornes avec un avertissement

    Plan du Scénario: <paramètre> demandé à <demandé> est ramené à <appliqué>
      Étant donné l'agent connecté à la base de test
      Quand l'agent exécute la requête "SELECT 1" avec <paramètre> à <demandé>
      Alors la requête réussit
      Et l'avertissement "<avertissement>" annonce <demandé> ramené à <appliqué>

      Exemples:
        | paramètre  | demandé | appliqué | avertissement     |
        | max_rows   | 900     | 500      | max_rows_clamped  |
        | max_rows   | 0       | 1        | max_rows_clamped  |
        | max_bytes  | 1000000 | 65536    | max_bytes_clamped |
        | max_bytes  | 0       | 1024     | max_bytes_clamped |
        | timeout_ms | 120000  | 60000    | timeout_clamped   |
        | timeout_ms | 10      | 100      | timeout_clamped   |

  @integration
  Règle: PostgreSQL : une réponse au-delà de max_rows est coupée et propose une pagination par clé

    Scénario: Sans limite demandée, une réponse compte au plus 50 lignes
      Étant donné l'agent connecté à la base de test
      Quand l'agent exécute la requête "SELECT id FROM big_numbers ORDER BY id"
      Alors la réponse compte 50 lignes
      Et la réponse est coupée par "rows" à 50
      Et l'indice de la coupure contient "WHERE id > last_id"

    Scénario: Une limite au-dessus du plafond rend 500 lignes
      Étant donné l'agent connecté à la base de test
      Quand l'agent exécute la requête "SELECT id FROM big_numbers ORDER BY id" avec max_rows à 900
      Alors la réponse compte 500 lignes
      Et la réponse est coupée par "rows" à 500

  @integration
  Règle: PostgreSQL : une réponse au-delà de max_bytes est coupée à son budget d'octets

    Scénario: Le budget par défaut est de 16 Ko et se demande jusqu'à 64 Ko
      Étant donné l'agent connecté à la base de test
      Quand l'agent exécute la requête "SELECT id, array_fill(7, ARRAY[200]) AS pad FROM big_numbers ORDER BY id"
      Alors la réponse est coupée par "bytes" à 16384
      Quand l'agent exécute la requête "SELECT id, array_fill(7, ARRAY[200]) AS pad FROM big_numbers ORDER BY id" avec max_rows à 500 et max_bytes à 1000000
      Alors la réponse est coupée par "bytes" à 65536

  @integration
  Règle: PostgreSQL : une valeur longue est coupée à 500 caractères et se lit par tranches avec cell_offset

    Scénario: Une cellule de 2000 caractères se lit en commençant au caractère demandé
      Étant donné l'agent connecté à la base de test
      Quand l'agent exécute la requête "SELECT repeat('Colis remis au transport ', 80) AS longue"
      Alors la première cellule compte 500 caractères suivis de "…[+1500 chars]"
      Et la réponse est coupée par "cell" à 500 sur la colonne "longue"
      Et l'indice de la coupure contient "cell_offset=500"
      Quand l'agent exécute la requête "SELECT repeat('Colis remis au transport ', 80) AS longue" avec cell_offset à 506
      Alors la première cellule commence par "remis au transport Colis"
      Et la réponse porte l'avertissement "cell_offset_applied"

    Scénario: Une agrégation coupée reste pseudonymisée
      Étant donné l'agent connecté à la base de test
      Quand l'agent exécute la requête "SELECT string_agg(c.email, ',') AS emails FROM customers c, generate_series(1, 40)"
      Alors la réponse est coupée par "cell" à 500 sur la colonne "emails"
      Et la réponse ne contient aucune valeur canari

  @integration
  Règle: PostgreSQL : une requête qui dépasse timeout_ms est annulée et la session reste utilisable
    Le délai s'applique à la requête de l'agent, que la session soit neuve ou déjà ouverte par un appel précédent.

    Scénario: Une requête trop longue sur une session déjà ouverte est signalée comme délai dépassé
      Étant donné l'agent connecté à la base de test
      Quand l'agent exécute la requête "SELECT 1"
      Alors la requête réussit
      Quand l'agent exécute la requête "SELECT count(*) FROM big_numbers a, big_numbers b" avec timeout_ms à 100
      Alors l'outil est refusé avec le code "statement_timeout"
      Et l'erreur peut être retentée
      Quand l'agent exécute la requête "SELECT 2"
      Alors la première cellule de la réponse vaut 2

  Règle: MongoDB : la lecture est bornée avant de partir vers le serveur
    Chaque pipeline reçoit un $limit de max_rows + 1 documents et un find lit au plus max_rows + 1 documents : le document de plus dit si la réponse est coupée.
    Une limite de find au-dessus de max_rows est ramenée à max_rows avec un avertissement,
    et l'exécution a un délai, n'écrit pas sur disque et lit de préférence un secondaire.

    Contexte:
      Étant donné l'agent connecté à une base MongoDB

    Scénario: Une agrégation de lecture reçoit la borne de documents
      Quand l'agent lance l'agrégation suivante :
        """
        [{"$match": {"status": "PAID"}}, {"$group": {"_id": "$status", "n": {"$sum": 1}}}]
        """
      Alors la demande est acceptée
      Et le pipeline envoyé se termine par une limite de 51 documents

    Scénario: Une limite de find au-dessus de max_rows est ramenée avec un avertissement
      Quand l'agent cherche avec une limite de 1000 documents
      Alors la recherche envoyée lit au plus 51 documents
      Et l'avertissement "limit_clamped" annonce 1000 ramené à 50

    Scénario: Une limite de find dans les bornes est gardée
      Quand l'agent cherche avec une limite de 10 documents
      Alors la recherche envoyée lit au plus 10 documents
      Et la demande n'est pas ramenée

    Scénario: L'exécution reprend le délai demandé
      Quand l'agent lance une agrégation avec un délai de 7000 ms
      Alors l'exécution est bornée à 7000 ms, sans écriture sur disque, sur un secondaire de préférence

  @integration
  Règle: MongoDB : un document trop long est coupé et se lit par tranches

    Scénario: Un texte long se lit par tranches et un tableau long est coupé à 100 éléments
      Étant donné la connexion MongoDB de test à travers le bastion
      Quand l'agent lit dans "customers" un texte de 2000 caractères et un tableau de 300 éléments
      Alors le champ "note" du premier document compte 500 caractères suivis de "…[+1500 chars]"
      Et le champ "items" du premier document compte 100 éléments
      Et la réponse est coupée par "cell" à 500 sur le champ "note"
      Et la réponse est coupée par "array" à 100 sur le champ "items"
      Quand l'agent relit le même texte avec cell_offset à 503
      Alors le champ "note" du premier document commence par "defghijabc"

    Scénario: Le budget d'octets coupe la réponse et propose une pagination sur _id
      Étant donné la connexion MongoDB de test à travers le bastion
      Quand l'agent cherche dans "customers" tous les documents avec max_bytes à 1024
      Alors la réponse est coupée par "bytes" à 1024
      Et l'indice de la coupure contient "$gt"
