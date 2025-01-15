#ifndef MFSMASTER_H
#define MFSMASTER_H

#include "chartsdefs.h"
#include "clocks.h"
#include "mfslog.h"

// Master server states
#define MASTER_STATE_LEADER 1
#define MASTER_STATE_FOLLOWER 2
#define MASTER_STATE_CANDIDATE 3

// Function prototypes
int get_my_id();
int get_quorum_size();
double monotonic_seconds();
void send_vote_response(int candidate_id, int term);
void request_votes();

#endif // MFSMASTER_H