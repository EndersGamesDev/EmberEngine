//! The stored row of control values a view box holds, and the path between two of them.

use ember_julibrot_math::{
    BigCentre, ObjectAngles, ViewControls, decode_big_scalar, encode_big_scalar, lerp_centre,
    lerp_f64, lerp_object_angles, lerp_origin, lerp_view, morph_precision_bits, round_centre,
};
use serde::{Deserialize, Serialize};

use crate::{AppError, PresetRow, ViewerController, state::NAVIGATION_PRECISION_BITS};

const PAGE_OBJECT_FIELDS: [&str; 6] = ["o12", "o13", "o14", "o23", "o24", "o34"];
const PAGE_CAMERA_FIELDS: [&str; 10] = [
    "q12", "q13", "q14", "q23", "q24", "q34", "q15", "q25", "q35", "q45",
];
const PAGE_TRANSLATION_FIELDS: [&str; 5] = ["t1", "t2", "t3", "t4", "t5"];

/// One coordinate of the authoritative centre in the form a view box stores.
///
/// The limbs are the bignum's own words. A saved view keeps them rather than the binary64 mirror
/// because a view saved past the mirror's reach would otherwise come back as a different picture,
/// which is the one thing a saved view must never do.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SavedCoordinate {
    /// Zero for a positive value, one for a negative one.
    pub sign: u32,
    /// Binary exponent of the lowest-order limb.
    pub exponent: i32,
    /// Little-endian magnitude words; empty is exact zero.
    pub limbs: Vec<u32>,
}

/// The authoritative centre with the precision it was captured at.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SavedCentre {
    /// Astro-float precision the coordinates were captured at.
    pub precision_bits: u32,
    /// The four ℝ⁴ coordinates in `(z.re, z.im, c.re, c.im)` order.
    pub coords: Vec<SavedCoordinate>,
}

impl SavedCentre {
    /// Decodes this stored centre back into the authoritative bignum.
    ///
    /// # Errors
    ///
    /// Returns a math failure for a malformed encoding or a coordinate count other than four.
    pub fn decode(&self) -> Result<BigCentre, AppError> {
        decode_centre(self)
    }
}

/// Everything that determines the picture except iteration cap and palette.
///
/// The field names are the preset row's names plus the two a preset has no opinion about, so the
/// page writes a preset and a saved view into its controls through one function.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedView {
    /// Six object angles in product order.
    pub object: [f64; 6],
    /// Absolute plane origin.
    pub origin: [f64; 4],
    /// Ten ambient-camera angles in product order.
    pub camera: [f64; 10],
    /// Five-dimensional camera translation applied before perspective.
    #[serde(default)]
    pub camera_translation: [f64; 5],
    /// Observer yaw in radians.
    pub camera_yaw: f64,
    /// Observer pitch in radians.
    pub camera_pitch: f64,
    /// Escape-height amplitude.
    pub height_scale: f64,
    /// Pole of the five-to-four perspective.
    pub distance_five: f64,
    /// Pole of the four-to-three perspective.
    pub distance_four: f64,
    /// Base-two zoom exponent, which the `scale` control carries.
    pub zoom_log2: f64,
    /// The authoritative centre in its canonical encoding.
    pub centre: SavedCentre,
    /// Finite mirror of the centre, for the readout only.
    ///
    /// It is never read back: loading and morphing both decode `centre`. It exists so that a box
    /// can show where it points without the page owning bignum arithmetic, and it is written
    /// beside the encoding rather than instead of it so the two can never be confused.
    pub centre_f64: [f64; 4],
}

impl SavedView {
    /// Serializes this row with each affine control as a page-addressable JSON key.
    ///
    /// # Errors
    ///
    /// Returns a serialization failure if the row cannot become the required object shape.
    pub fn to_page_json(&self) -> Result<String, AppError> {
        let mut value = serde_json::to_value(self).map_err(|error| serialization(&error))?;
        let row = value
            .as_object_mut()
            .ok_or_else(|| AppError::Serialization("saved row is not a JSON object".to_string()))?;
        flatten_page_array(row, "object", &PAGE_OBJECT_FIELDS)?;
        flatten_page_array(row, "camera", &PAGE_CAMERA_FIELDS)?;
        flatten_page_array(row, "camera_translation", &PAGE_TRANSLATION_FIELDS)?;
        serde_json::to_string(&value).map_err(|error| serialization(&error))
    }

    /// Decodes the page's flat affine-control JSON into the stored row shape.
    ///
    /// # Errors
    ///
    /// Returns a serialization failure for malformed JSON or a missing affine control.
    pub fn from_page_json(json: &str) -> Result<Self, AppError> {
        let mut value: serde_json::Value =
            serde_json::from_str(json).map_err(|error| serialization(&error))?;
        let row = value
            .as_object_mut()
            .ok_or_else(|| AppError::Serialization("saved row is not a JSON object".to_string()))?;
        expand_page_array(row, "object", &PAGE_OBJECT_FIELDS)?;
        expand_page_array(row, "camera", &PAGE_CAMERA_FIELDS)?;
        expand_page_array(row, "camera_translation", &PAGE_TRANSLATION_FIELDS)?;
        serde_json::from_value(value).map_err(|error| serialization(&error))
    }

    /// Expands a built-in row into the complete stored-row shape used by the atomic boundary.
    ///
    /// # Errors
    ///
    /// Returns a math failure if the preset origin cannot form the canonical exact centre.
    pub fn from_preset(row: PresetRow) -> Result<Self, AppError> {
        let centre = BigCentre::from_f64(row.plane_origin, NAVIGATION_PRECISION_BITS)
            .map_err(|error| math(&error))?;
        Ok(Self {
            object: row.object_angles.as_array(),
            origin: row.plane_origin,
            camera: row.view.camera,
            camera_translation: row.view.camera_translation,
            camera_yaw: row.view.camera_yaw,
            camera_pitch: row.view.camera_pitch,
            height_scale: row.view.height_scale,
            distance_five: row.view.distance_five,
            distance_four: row.view.distance_four,
            zoom_log2: 0.0,
            centre_f64: centre.to_f64_mirror(),
            centre: encode_centre(&centre)?,
        })
    }

    /// Captures the row the viewer is currently showing.
    ///
    /// # Errors
    ///
    /// Returns a math failure when navigation is unconfigured or a coordinate does not encode.
    pub fn capture(viewer: &ViewerController) -> Result<Self, AppError> {
        let requested = viewer.requested();
        let centre = viewer
            .owner()
            .navigation_centre()
            .ok_or_else(|| AppError::Math("navigation is unconfigured".to_string()))?;
        Ok(Self {
            object: requested.object_angles.as_array(),
            origin: requested.plane_origin,
            camera: requested.view.camera,
            camera_translation: requested.view.camera_translation,
            camera_yaw: requested.view.camera_yaw,
            camera_pitch: requested.view.camera_pitch,
            height_scale: requested.view.height_scale,
            distance_five: requested.view.distance_five,
            distance_four: requested.view.distance_four,
            zoom_log2: requested.zoom_log2,
            centre_f64: centre.to_f64_mirror(),
            centre: encode_centre(&centre)?,
        })
    }

    /// Returns the twenty view controls this row carries.
    #[must_use]
    pub const fn view(&self) -> ViewControls {
        ViewControls {
            camera: self.camera,
            camera_translation: self.camera_translation,
            camera_yaw: self.camera_yaw,
            camera_pitch: self.camera_pitch,
            height_scale: self.height_scale,
            distance_five: self.distance_five,
            distance_four: self.distance_four,
        }
    }

    /// Returns the six object angles this row carries.
    #[must_use]
    pub const fn object_angles(&self) -> ObjectAngles {
        ObjectAngles {
            rho_12: self.object[0],
            rho_13: self.object[1],
            rho_14: self.object[2],
            rho_23: self.object[3],
            rho_24: self.object[4],
            rho_34: self.object[5],
        }
    }

    /// Decodes the stored centre back into the authoritative bignum.
    ///
    /// # Errors
    ///
    /// Returns a math failure for a malformed encoding or a coordinate count other than four.
    pub fn centre(&self) -> Result<BigCentre, AppError> {
        decode_centre(&self.centre)
    }

    /// Interpolates one saved row into another, composing math's interpolators only.
    ///
    /// No arithmetic happens here: every scalar and the centre are math's to move, and this
    /// function exists so that the app has one row-shaped name for the whole of it.
    ///
    /// # Errors
    ///
    /// Returns a math failure for a `t` outside `[0,1]`, a malformed centre, or a refused row.
    pub fn lerp(from: &Self, to: &Self, t: f64) -> Result<Self, AppError> {
        let object = lerp_object_angles(from.object_angles(), to.object_angles(), t)
            .map_err(|error| math(&error))?;
        let view = lerp_view(from.view(), to.view(), t).map_err(|error| math(&error))?;
        let origin = lerp_origin(from.origin, to.origin, t).map_err(|error| math(&error))?;
        let zoom_log2 = lerp_f64(from.zoom_log2, to.zoom_log2, t).map_err(|error| math(&error))?;
        let first = from.centre()?;
        let second = to.centre()?;
        let working_bits = morph_precision_bits(&first, &second).map_err(|error| math(&error))?;
        // The extra bits are the arithmetic's, not the row's. A row is installed as the viewer's
        // own centre and its reference, and displacement against a reference is refused when the
        // two precisions differ, so a row handed back at working precision stops the loop on the
        // first step of the slider. Rounding back to the deeper endpoint is exact for both ends.
        let precision_bits = first.precision_bits.max(second.precision_bits);
        let centre = round_centre(
            &lerp_centre(&first, &second, t, working_bits).map_err(|error| math(&error))?,
            precision_bits,
        )
        .map_err(|error| math(&error))?;
        Ok(Self {
            object: object.as_array(),
            origin,
            camera: view.camera,
            camera_translation: view.camera_translation,
            camera_yaw: view.camera_yaw,
            camera_pitch: view.camera_pitch,
            height_scale: view.height_scale,
            distance_five: view.distance_five,
            distance_four: view.distance_four,
            zoom_log2,
            centre_f64: centre.to_f64_mirror(),
            centre: encode_centre(&centre)?,
        })
    }
}

fn flatten_page_array(
    row: &mut serde_json::Map<String, serde_json::Value>,
    source: &str,
    fields: &[&str],
) -> Result<(), AppError> {
    let Some(serde_json::Value::Array(values)) = row.remove(source) else {
        return Err(AppError::Serialization(
            "saved row has no affine control array".to_string(),
        ));
    };
    if values.len() != fields.len() {
        return Err(AppError::Serialization(
            "saved row affine control count differs".to_string(),
        ));
    }
    for (field, value) in fields.iter().zip(values) {
        row.insert((*field).to_string(), value);
    }
    Ok(())
}

fn expand_page_array(
    row: &mut serde_json::Map<String, serde_json::Value>,
    target: &str,
    fields: &[&str],
) -> Result<(), AppError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(fields.len())
        .map_err(|_| AppError::Serialization("saved row affine controls do not fit".to_string()))?;
    for field in fields {
        values.push(row.remove(*field).ok_or_else(|| {
            AppError::Serialization("saved row affine control is missing".to_string())
        })?);
    }
    row.insert(target.to_string(), serde_json::Value::Array(values));
    Ok(())
}

fn serialization(error: &serde_json::Error) -> AppError {
    AppError::Serialization(error.to_string())
}

fn encode_centre(centre: &BigCentre) -> Result<SavedCentre, AppError> {
    let mut coords = Vec::new();
    coords
        .try_reserve_exact(centre.coords.len())
        .map_err(|_| AppError::Math("saved centre does not fit".to_string()))?;
    for coordinate in &centre.coords {
        let encoded = encode_big_scalar(coordinate).map_err(|error| math(&error))?;
        coords.push(SavedCoordinate {
            sign: encoded.sign,
            exponent: encoded.exponent,
            limbs: encoded.limbs,
        });
    }
    Ok(SavedCentre {
        precision_bits: centre.precision_bits,
        coords,
    })
}

fn decode_centre(saved: &SavedCentre) -> Result<BigCentre, AppError> {
    if saved.coords.len() != 4 {
        return Err(AppError::Math(
            "saved centre does not carry four coordinates".to_string(),
        ));
    }
    let mut decoded = Vec::new();
    decoded
        .try_reserve_exact(4)
        .map_err(|_| AppError::Math("saved centre does not fit".to_string()))?;
    for coordinate in &saved.coords {
        decoded.push(
            decode_big_scalar(
                coordinate.sign,
                coordinate.exponent,
                &coordinate.limbs,
                saved.precision_bits,
            )
            .map_err(|error| math(&error))?,
        );
    }
    let coords: [_; 4] = decoded
        .try_into()
        .map_err(|_| AppError::Math("saved centre does not carry four coordinates".to_string()))?;
    Ok(BigCentre {
        coords,
        precision_bits: saved.precision_bits,
    })
}

fn math(error: &ember_julibrot_math::MathError) -> AppError {
    AppError::Math(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deep_viewer() -> ViewerController {
        let mut viewer = ViewerController::new([960, 540]).expect("canonical viewer");
        viewer
            .set_zoom_log2(24.0)
            .expect("a scale inside the range");
        viewer
            .set_crosshair([137.0, -64.0])
            .expect("a finite target");
        viewer
    }

    fn finish_initial_reference(viewer: &mut ViewerController) {
        let initial = viewer
            .take_reference_submission()
            .expect("the canonical view requests one initial reference");
        assert!(viewer.finish_reference_submission(initial.navigation.generation));
        assert!(viewer.take_reference_submission().is_none());
    }

    /// A captured row must come back through its own JSON without moving a bit.
    #[test]
    fn a_captured_row_round_trips_through_its_json() {
        let viewer = deep_viewer();
        let saved = SavedView::capture(&viewer).expect("a capturable row");
        let text = serde_json::to_string(&saved).expect("a serializable row");
        let restored: SavedView = serde_json::from_str(&text).expect("a decodable row");
        assert_eq!(restored, saved);
        let centre = saved.centre().expect("a decodable centre");
        let again = SavedView {
            centre: encode_centre(&centre).expect("an encodable centre"),
            ..restored
        };
        assert_eq!(again, saved);
    }

    /// A deep centre must survive the encoding exactly, which is the whole reason it is stored.
    #[test]
    fn a_deep_centre_survives_the_encoding_it_is_stored_in() {
        let viewer = deep_viewer();
        let saved = SavedView::capture(&viewer).expect("a capturable row");
        let decoded = saved.centre().expect("a decodable centre");
        let original = viewer
            .owner()
            .navigation_centre()
            .expect("configured navigation");
        assert_eq!(decoded.precision_bits, original.precision_bits);
        assert_eq!(decoded.to_f64_mirror(), original.to_f64_mirror());
        assert_eq!(decoded, original);
        assert!(saved.zoom_log2 > 20.0, "the fixture is not a deep view");
    }

    /// Both ends of the morph must be the rows the boxes hold, including their centres.
    #[test]
    fn the_morph_returns_each_box_at_its_own_end() {
        let first = SavedView::capture(&deep_viewer()).expect("a capturable row");
        let mut other = ViewerController::new([960, 540]).expect("canonical viewer");
        other
            .set_plane_origin([0.0, 0.0, -0.8, 0.156])
            .expect("a finite origin");
        other.set_zoom_log2(3.0).expect("a scale inside the range");
        let second = SavedView::capture(&other).expect("a capturable row");

        let start = SavedView::lerp(&first, &second, 0.0).expect("a finite morph");
        let end = SavedView::lerp(&first, &second, 1.0).expect("a finite morph");
        for (morphed, original) in [(&start, &first), (&end, &second)] {
            assert_eq!(morphed.view(), original.view());
            assert_eq!(morphed.object_angles(), original.object_angles());
            assert_eq!(morphed.origin, original.origin);
            assert_eq!(morphed.zoom_log2, original.zoom_log2);
            assert_eq!(
                morphed.centre().expect("decodable end").to_f64_mirror(),
                original.centre().expect("decodable end").to_f64_mirror()
            );
        }
    }

    /// A morph with both boxes holding the same row must not move anything.
    #[test]
    fn a_morph_between_one_row_and_itself_is_that_row() {
        let row = SavedView::capture(&deep_viewer()).expect("a capturable row");
        for t in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let morphed = SavedView::lerp(&row, &row, t).expect("a finite morph");
            assert_eq!(morphed.view(), row.view());
            assert_eq!(morphed.origin, row.origin);
            assert_eq!(morphed.zoom_log2, row.zoom_log2);
            assert_eq!(
                morphed.centre().expect("decodable row").to_f64_mirror(),
                row.centre().expect("decodable row").to_f64_mirror()
            );
        }
    }

    /// The morph refuses what it cannot mean rather than guessing a row.
    #[test]
    fn a_fraction_outside_the_slider_or_a_malformed_centre_is_refused() {
        let row = SavedView::capture(&deep_viewer()).expect("a capturable row");
        assert!(SavedView::lerp(&row, &row, -0.1).is_err());
        assert!(SavedView::lerp(&row, &row, 1.1).is_err());
        let mut broken = row.clone();
        broken.centre.coords.pop();
        assert!(broken.centre().is_err());
        assert!(SavedView::lerp(&row, &broken, 0.5).is_err());
    }

    /// Characterizes the new atomic path: pose-only morphing touches neither MAIN nor reference.
    #[test]
    fn a_same_origin_pose_morph_is_one_reference_free_transaction() {
        let mut viewer = ViewerController::new([960, 540]).expect("canonical viewer");
        finish_initial_reference(&mut viewer);
        let first = SavedView::capture(&viewer).expect("first row");
        let mut second = first.clone();
        second.camera[0] = 0.35;
        second.camera_translation[4] = -0.2;
        second.camera_yaw = 0.125;
        second.height_scale = 0.75;
        let row = SavedView::lerp(&first, &second, 0.5).expect("pose morph");
        let rebuilds = viewer.main_state_rebuild_count();
        let revision = viewer.requested_revision();

        viewer.apply_saved_view(&row).expect("atomic row");

        assert_eq!(viewer.main_state_rebuild_count(), rebuilds);
        assert_eq!(viewer.requested_revision(), revision + 1);
        assert!(viewer.take_reference_submission().is_none());
        assert_eq!(viewer.requested().view, row.view());
    }

    /// Characterizes the new atomic path: a slice crossing releases one coalesced request.
    #[test]
    fn a_cross_slice_morph_rebuilds_and_requests_exactly_once() {
        let mut viewer = ViewerController::new([960, 540]).expect("canonical viewer");
        finish_initial_reference(&mut viewer);
        let first = SavedView::capture(&viewer).expect("first row");
        let mut other = ViewerController::new([960, 540]).expect("other viewer");
        other
            .set_plane_origin([0.25, 0.0, 0.0, 0.0])
            .expect("other slice");
        let second = SavedView::capture(&other).expect("second row");
        let row = SavedView::lerp(&first, &second, 0.5).expect("slice morph");

        viewer.apply_saved_view(&row).expect("atomic row");

        assert_eq!(viewer.main_state_rebuild_count(), 1);
        assert_eq!(viewer.requested_revision(), 1);
        let request = viewer
            .take_reference_submission()
            .expect("one cross-slice request");
        assert!(viewer.finish_reference_submission(request.navigation.generation));
        assert!(viewer.take_reference_submission().is_none());
        assert_eq!(request.reason, ember_julibrot_worker::OrbitReason::INITIAL);
    }

    /// The public origin and centre setters are no-ops for bit-identical requested values.
    #[test]
    fn equal_origin_and_centre_setters_do_not_rebuild_or_request() {
        let mut viewer = ViewerController::new([960, 540]).expect("canonical viewer");
        finish_initial_reference(&mut viewer);
        let centre = viewer
            .owner()
            .navigation_centre()
            .expect("configured centre");

        viewer
            .set_plane_origin(viewer.requested().plane_origin)
            .expect("equal origin");
        viewer.set_centre(centre).expect("equal centre");

        assert_eq!(viewer.main_state_rebuild_count(), 0);
        assert_eq!(viewer.requested_revision(), 0);
        assert!(viewer.take_reference_submission().is_none());
    }

    /// A centre guard repairs a mismatched reference even when the navigation centre is equal.
    #[test]
    fn centre_setter_reinstalls_an_equal_navigation_centre_as_its_reference() {
        let mut viewer = ViewerController::new([960, 540]).expect("canonical viewer");
        finish_initial_reference(&mut viewer);
        viewer.pan_px([24.0, 0.0]).expect("finite pan");
        let centre = viewer
            .owner()
            .navigation_centre()
            .expect("configured centre");
        assert_ne!(viewer.owner().reference_centre(), Some(centre.clone()));

        viewer.set_centre(centre.clone()).expect("centre repair");

        assert_eq!(viewer.owner().navigation_centre(), Some(centre.clone()));
        assert_eq!(viewer.owner().reference_centre(), Some(centre));
        assert_eq!(
            viewer
                .take_reference_submission()
                .expect("one repaired-centre request")
                .reason,
            ember_julibrot_worker::OrbitReason::CENTRE_THRESHOLD
        );
    }
}
