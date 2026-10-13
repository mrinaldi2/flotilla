//! Attachments and drafts from the owner's apps (0.19.0 K4b): uploading a
//! file in chunks and attaching it, linking a document, reading an
//! attachment's bytes, and a draft's invitations, revisions, finalize and
//! discard.

use bus::contract::board::{self as c, board_request::Request, board_response::Response};

use super::item_edits::refusal;
use super::reads::item_detail;
use super::{refuse, Refusal};
use crate::kanban::attach::{read_chunk, Source};
use crate::kanban::edit::Edited;
use crate::kanban::moves::Moved;
use crate::kanban::{draft_undo, drafts};
use crate::ws::Conn;

impl Conn {
    pub(super) fn draft_request(&self, req: Request) -> Result<Response, Refusal> {
        // Only the owner finalizes, discards, invites to or marks seen a
        // draft (decision 5). The owner token is one a bot of the same user
        // can read, so these take the app's ticket or a paired device, as
        // every act of the owner's own does (`owner_auth`).
        let owners_own = match &req {
            Request::ItemInvite(_) | Request::ItemFinalize(_) | Request::ItemDiscard(_) => true,
            Request::ItemDraftChanges(r) => r.mark_seen == Some(true),
            _ => false,
        };
        if owners_own && self.owner_proof().is_none() {
            return Err(refuse(
                "forbidden",
                "only the owner, from the app or a paired device, invites to, finalizes, \
                 discards or marks seen a draft; not with the owner token",
            ));
        }
        let actor = self.actor();
        Ok(match req {
            Request::AttachmentUpload(r) => {
                let project_id = self.project_of_item(&r.item_id)?;
                let received = self.editor(&project_id).upload(&r).map_err(refusal)?;
                Response::Upload(c::UploadProgress {
                    upload_id: r.upload_id,
                    received,
                })
            }
            Request::ItemAttach(r) => {
                let source = match (&r.upload_id, &r.artifact, &r.url, &r.file) {
                    (Some(upload), None, None, None) => Source::Upload(upload.clone()),
                    (None, Some(artifact), None, None) => Source::Artifact(artifact.clone()),
                    (None, None, Some(url), None) => Source::Url(url.clone()),
                    (None, None, None, Some(_)) => {
                        return Err(refuse(
                            "invalid_request",
                            "upload the file with attachment_upload, then attach the upload",
                        ))
                    }
                    _ => {
                        return Err(refuse(
                            "invalid_request",
                            "give one of 'upload_id', 'artifact' or 'url'",
                        ))
                    }
                };
                let project_id = self.project_of_item(&r.id)?;
                self.editor(&project_id)
                    .attach(&r, source)
                    .map_err(refusal)?;
                Response::Item(item_detail(&self.app, &r.id)?)
            }
            Request::ItemDetach(r) => {
                let project_id = self.project_of_item(&r.id)?;
                self.editor(&project_id).detach(&r).map_err(refusal)?;
                Response::Item(item_detail(&self.app, &r.id)?)
            }
            Request::AttachmentRead(r) => {
                Response::Attachment(read_chunk(&self.app, &r).map_err(refusal)?)
            }
            Request::ItemInvite(r) => {
                drafts::invite(&self.app, &actor, &r).map_err(refusal)?;
                Response::Item(item_detail(&self.app, &r.id)?)
            }
            Request::ItemFinalize(r) => Response::Moved(move_result(
                drafts::finalize(&self.app, &actor, &r).map_err(refusal)?,
            )),
            Request::ItemDiscard(r) => Response::Edited(edit_result(
                drafts::discard(&self.app, &actor, &r).map_err(refusal)?,
            )),
            Request::ItemDraftUndo(r) => Response::Edited(edit_result(
                draft_undo::undo(&self.app, &actor, &r).map_err(refusal)?,
            )),
            Request::ItemDraftChanges(r) => {
                Response::DraftChanges(drafts::changes(&self.app, &r).map_err(refusal)?)
            }
            _ => {
                return Err(refuse(
                    "invalid_request",
                    "not an attachment or draft request",
                ))
            }
        })
    }
}

pub(super) fn move_result(moved: Moved) -> c::MoveResult {
    use c::move_result::Outcome;
    c::MoveResult {
        outcome: Some(match moved {
            Moved::Done(item) => Outcome::Done((*item).into()),
            Moved::Refused(unmet) => Outcome::Refused(c::MoveRefused {
                unmet: unmet.into_iter().map(Into::into).collect(),
            }),
            Moved::Conflict(item) => Outcome::Conflict((*item).into()),
        }),
    }
}

pub(super) fn edit_result(edited: Edited) -> c::EditResult {
    use c::edit_result::Outcome;
    c::EditResult {
        outcome: Some(match edited {
            Edited::Done(item) => Outcome::Done((*item).into()),
            Edited::Refused(unmet) => Outcome::Refused(c::MoveRefused {
                unmet: unmet.into_iter().map(Into::into).collect(),
            }),
            Edited::Conflict(item) => Outcome::Conflict((*item).into()),
        }),
        told: Vec::new(),
    }
}
