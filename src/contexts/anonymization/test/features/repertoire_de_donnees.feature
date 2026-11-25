# language: fr
@anonymization
Fonctionnalité: Choix du répertoire de données
  Le secret et le magasin vivent dans un répertoire de données propre à l'utilisateur. Un répertoire explicite l'emporte
  toujours ; sinon le serveur prend son dossier sous la racine des données de l'utilisateur, en gardant l'ancien dossier
  de pg-pseudo-mcp quand le nouveau n'existe pas ou que l'ancien porte le secret, pour que les pseudonymes déjà donnés
  restent valables. Quand les deux dossiers existent, le serveur le signale par l'avis data_directory_conflict.

  Règle: Un répertoire de données explicite l'emporte toujours

    Scénario: Le répertoire explicite est utilisé même quand les deux dossiers portent un secret
      Étant donné une racine de données
      Et le dossier "scarif-database-mcp" porte un secret
      Et le dossier "pg-pseudo-mcp" porte un secret
      Et un répertoire de données explicite "explicite"
      Quand le serveur choisit son répertoire de données
      Alors il utilise le dossier "explicite"
      Et il ne signale aucun conflit

  Règle: Sans répertoire explicite, le dossier scarif-database-mcp est le dossier par défaut

    Scénario: Sur un poste neuf, le dossier scarif-database-mcp est choisi
      Étant donné une racine de données
      Quand le serveur choisit son répertoire de données
      Alors il utilise le dossier "scarif-database-mcp"
      Et il ne signale aucun conflit

    Scénario: Le dossier scarif-database-mcp est choisi quand il porte le secret, même si l'ancien existe
      Étant donné une racine de données
      Et le dossier "scarif-database-mcp" porte un secret
      Et le dossier "pg-pseudo-mcp" existe sans secret
      Quand le serveur choisit son répertoire de données
      Alors il utilise le dossier "scarif-database-mcp"

  Règle: L'ancien dossier pg-pseudo-mcp est gardé tant que le nouveau n'existe pas ou qu'il porte le secret

    Scénario: Seul l'ancien dossier existe : il est gardé, sans rien créer ni déplacer
      Étant donné une racine de données
      Et le dossier "pg-pseudo-mcp" porte un secret
      Quand le serveur choisit son répertoire de données
      Alors il utilise le dossier "pg-pseudo-mcp"
      Et il ne signale aucun conflit
      Et le dossier "scarif-database-mcp" n'existe pas
      Et un pseudonyme émis dans l'ancien dossier reste identique

    Scénario: Un ancien dossier sans secret est gardé quand le nouveau n'existe pas
      Étant donné une racine de données
      Et le dossier "pg-pseudo-mcp" existe sans secret
      Quand le serveur choisit son répertoire de données
      Alors il utilise le dossier "pg-pseudo-mcp"
      Et le dossier "scarif-database-mcp" n'existe pas

    Plan du Scénario: Un nouveau dossier <état> ne remplace pas l'ancien qui porte le secret
      Étant donné une racine de données
      Et le dossier "pg-pseudo-mcp" porte un secret
      Et le dossier "scarif-database-mcp" <préparation>
      Quand le serveur choisit son répertoire de données
      Alors il utilise le dossier "pg-pseudo-mcp"
      Et il signale un conflit
      Et un pseudonyme émis dans l'ancien dossier reste identique

      Exemples:
        | état                                 | préparation        |
        | sans secret                          | existe sans secret |
        | créé par erreur avec son propre secret | porte un secret  |

  Règle: Sans répertoire explicite ni racine de données, le serveur ne choisit aucun dossier

    Scénario: Aucun dossier ne peut être déterminé
      Quand le serveur choisit son répertoire de données
      Alors aucun répertoire de données n'est choisi
