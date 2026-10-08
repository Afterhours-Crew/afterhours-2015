// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! typed definition-class bindings and derived fields, integrated in.
//! Scalar words retain their exact bits; numeric domains are not inferred.
//! The shared Tire/Brake serializers use the same layout for two classes.
use super::{Error, Reader, Writer};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefinitionClass {
    AirFilterItemData,
    BodyKitItemData,
    BrakeDiscsItemData,
    BrakesItemData,
    BumperItemData,
    CalipersItemData,
    CamShaftItemData,
    CanardItemData,
    CategoryUnlockControllerItemData,
    ClutchItemData,
    ControlArmTuningItemData,
    CurrencyItemData,
    CylinderHeadsItemData,
    DifferentialTuningItemData,
    DiffuserItemData,
    DiscountItemData,
    EcuItemData,
    ElectricSystemItemData,
    EngineBlockItemData,
    EngineDisplacementItemData,
    ExhaustItemData,
    ExhaustManifoldItemData,
    ExhaustSystemItemData,
    FendersItemData,
    ForcedInductionItemData,
    FuelSystemItemData,
    GearboxTuningItemData,
    GearsItemData,
    HandbrakeTuningItemData,
    HoodItemData,
    IgnitionItemData,
    IntakeManifoldItemData,
    LicensePlateBackgroundItemData,
    LicensePlateFrameItemData,
    LightsItemData,
    LiveryCustomizationItemData,
    LiveryDecalSwatchPackItemData,
    NosTuningItemData,
    PersistantTuningItemData,
    RaceVehicleItemData,
    RadiatorItemData,
    RimsItemData,
    RollCageItemData,
    RoofItemData,
    SideSkirtsItemData,
    SoundSystemItemData,
    SplitterItemData,
    SpoilerItemData,
    StaticLiveryCustomizationItemData,
    SteeringTuningItemData,
    StyleItemData,
    SuspensionItemData,
    SuspensionTuningItemData,
    SwaybarTuningItemData,
    TimedDiscountItemData,
    TireCompositionItemData,
    TiresItemData,
    TrunkLidItemData,
    WingMirrorsItemData,
}
impl DefinitionClass {
    pub const ALL: [Self; 59] = [
        Self::AirFilterItemData,
        Self::BodyKitItemData,
        Self::BrakeDiscsItemData,
        Self::BrakesItemData,
        Self::BumperItemData,
        Self::CalipersItemData,
        Self::CamShaftItemData,
        Self::CanardItemData,
        Self::CategoryUnlockControllerItemData,
        Self::ClutchItemData,
        Self::ControlArmTuningItemData,
        Self::CurrencyItemData,
        Self::CylinderHeadsItemData,
        Self::DifferentialTuningItemData,
        Self::DiffuserItemData,
        Self::DiscountItemData,
        Self::EcuItemData,
        Self::ElectricSystemItemData,
        Self::EngineBlockItemData,
        Self::EngineDisplacementItemData,
        Self::ExhaustItemData,
        Self::ExhaustManifoldItemData,
        Self::ExhaustSystemItemData,
        Self::FendersItemData,
        Self::ForcedInductionItemData,
        Self::FuelSystemItemData,
        Self::GearboxTuningItemData,
        Self::GearsItemData,
        Self::HandbrakeTuningItemData,
        Self::HoodItemData,
        Self::IgnitionItemData,
        Self::IntakeManifoldItemData,
        Self::LicensePlateBackgroundItemData,
        Self::LicensePlateFrameItemData,
        Self::LightsItemData,
        Self::LiveryCustomizationItemData,
        Self::LiveryDecalSwatchPackItemData,
        Self::NosTuningItemData,
        Self::PersistantTuningItemData,
        Self::RaceVehicleItemData,
        Self::RadiatorItemData,
        Self::RimsItemData,
        Self::RollCageItemData,
        Self::RoofItemData,
        Self::SideSkirtsItemData,
        Self::SoundSystemItemData,
        Self::SplitterItemData,
        Self::SpoilerItemData,
        Self::StaticLiveryCustomizationItemData,
        Self::SteeringTuningItemData,
        Self::StyleItemData,
        Self::SuspensionItemData,
        Self::SuspensionTuningItemData,
        Self::SwaybarTuningItemData,
        Self::TimedDiscountItemData,
        Self::TireCompositionItemData,
        Self::TiresItemData,
        Self::TrunkLidItemData,
        Self::WingMirrorsItemData,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::AirFilterItemData => "AirFilterItemData",
            Self::BodyKitItemData => "BodyKitItemData",
            Self::BrakeDiscsItemData => "BrakeDiscsItemData",
            Self::BrakesItemData => "BrakesItemData",
            Self::BumperItemData => "BumperItemData",
            Self::CalipersItemData => "CalipersItemData",
            Self::CamShaftItemData => "CamShaftItemData",
            Self::CanardItemData => "CanardItemData",
            Self::CategoryUnlockControllerItemData => "CategoryUnlockControllerItemData",
            Self::ClutchItemData => "ClutchItemData",
            Self::ControlArmTuningItemData => "ControlArmTuningItemData",
            Self::CurrencyItemData => "CurrencyItemData",
            Self::CylinderHeadsItemData => "CylinderHeadsItemData",
            Self::DifferentialTuningItemData => "DifferentialTuningItemData",
            Self::DiffuserItemData => "DiffuserItemData",
            Self::DiscountItemData => "DiscountItemData",
            Self::EcuItemData => "EcuItemData",
            Self::ElectricSystemItemData => "ElectricSystemItemData",
            Self::EngineBlockItemData => "EngineBlockItemData",
            Self::EngineDisplacementItemData => "EngineDisplacementItemData",
            Self::ExhaustItemData => "ExhaustItemData",
            Self::ExhaustManifoldItemData => "ExhaustManifoldItemData",
            Self::ExhaustSystemItemData => "ExhaustSystemItemData",
            Self::FendersItemData => "FendersItemData",
            Self::ForcedInductionItemData => "ForcedInductionItemData",
            Self::FuelSystemItemData => "FuelSystemItemData",
            Self::GearboxTuningItemData => "GearboxTuningItemData",
            Self::GearsItemData => "GearsItemData",
            Self::HandbrakeTuningItemData => "HandbrakeTuningItemData",
            Self::HoodItemData => "HoodItemData",
            Self::IgnitionItemData => "IgnitionItemData",
            Self::IntakeManifoldItemData => "IntakeManifoldItemData",
            Self::LicensePlateBackgroundItemData => "LicensePlateBackgroundItemData",
            Self::LicensePlateFrameItemData => "LicensePlateFrameItemData",
            Self::LightsItemData => "LightsItemData",
            Self::LiveryCustomizationItemData => "LiveryCustomizationItemData",
            Self::LiveryDecalSwatchPackItemData => "LiveryDecalSwatchPackItemData",
            Self::NosTuningItemData => "NosTuningItemData",
            Self::PersistantTuningItemData => "PersistantTuningItemData",
            Self::RaceVehicleItemData => "RaceVehicleItemData",
            Self::RadiatorItemData => "RadiatorItemData",
            Self::RimsItemData => "RimsItemData",
            Self::RollCageItemData => "RollCageItemData",
            Self::RoofItemData => "RoofItemData",
            Self::SideSkirtsItemData => "SideSkirtsItemData",
            Self::SoundSystemItemData => "SoundSystemItemData",
            Self::SplitterItemData => "SplitterItemData",
            Self::SpoilerItemData => "SpoilerItemData",
            Self::StaticLiveryCustomizationItemData => "StaticLiveryCustomizationItemData",
            Self::SteeringTuningItemData => "SteeringTuningItemData",
            Self::StyleItemData => "StyleItemData",
            Self::SuspensionItemData => "SuspensionItemData",
            Self::SuspensionTuningItemData => "SuspensionTuningItemData",
            Self::SwaybarTuningItemData => "SwaybarTuningItemData",
            Self::TimedDiscountItemData => "TimedDiscountItemData",
            Self::TireCompositionItemData => "TireCompositionItemData",
            Self::TiresItemData => "TiresItemData",
            Self::TrunkLidItemData => "TrunkLidItemData",
            Self::WingMirrorsItemData => "WingMirrorsItemData",
        }
    }
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "AirFilterItemData" => Some(Self::AirFilterItemData),
            "BodyKitItemData" => Some(Self::BodyKitItemData),
            "BrakeDiscsItemData" => Some(Self::BrakeDiscsItemData),
            "BrakesItemData" => Some(Self::BrakesItemData),
            "BumperItemData" => Some(Self::BumperItemData),
            "CalipersItemData" => Some(Self::CalipersItemData),
            "CamShaftItemData" => Some(Self::CamShaftItemData),
            "CanardItemData" => Some(Self::CanardItemData),
            "CategoryUnlockControllerItemData" => Some(Self::CategoryUnlockControllerItemData),
            "ClutchItemData" => Some(Self::ClutchItemData),
            "ControlArmTuningItemData" => Some(Self::ControlArmTuningItemData),
            "CurrencyItemData" => Some(Self::CurrencyItemData),
            "CylinderHeadsItemData" => Some(Self::CylinderHeadsItemData),
            "DifferentialTuningItemData" => Some(Self::DifferentialTuningItemData),
            "DiffuserItemData" => Some(Self::DiffuserItemData),
            "DiscountItemData" => Some(Self::DiscountItemData),
            "EcuItemData" => Some(Self::EcuItemData),
            "ElectricSystemItemData" => Some(Self::ElectricSystemItemData),
            "EngineBlockItemData" => Some(Self::EngineBlockItemData),
            "EngineDisplacementItemData" => Some(Self::EngineDisplacementItemData),
            "ExhaustItemData" => Some(Self::ExhaustItemData),
            "ExhaustManifoldItemData" => Some(Self::ExhaustManifoldItemData),
            "ExhaustSystemItemData" => Some(Self::ExhaustSystemItemData),
            "FendersItemData" => Some(Self::FendersItemData),
            "ForcedInductionItemData" => Some(Self::ForcedInductionItemData),
            "FuelSystemItemData" => Some(Self::FuelSystemItemData),
            "GearboxTuningItemData" => Some(Self::GearboxTuningItemData),
            "GearsItemData" => Some(Self::GearsItemData),
            "HandbrakeTuningItemData" => Some(Self::HandbrakeTuningItemData),
            "HoodItemData" => Some(Self::HoodItemData),
            "IgnitionItemData" => Some(Self::IgnitionItemData),
            "IntakeManifoldItemData" => Some(Self::IntakeManifoldItemData),
            "LicensePlateBackgroundItemData" => Some(Self::LicensePlateBackgroundItemData),
            "LicensePlateFrameItemData" => Some(Self::LicensePlateFrameItemData),
            "LightsItemData" => Some(Self::LightsItemData),
            "LiveryCustomizationItemData" => Some(Self::LiveryCustomizationItemData),
            "LiveryDecalSwatchPackItemData" => Some(Self::LiveryDecalSwatchPackItemData),
            "NosTuningItemData" => Some(Self::NosTuningItemData),
            "PersistantTuningItemData" => Some(Self::PersistantTuningItemData),
            "RaceVehicleItemData" => Some(Self::RaceVehicleItemData),
            "RadiatorItemData" => Some(Self::RadiatorItemData),
            "RimsItemData" => Some(Self::RimsItemData),
            "RollCageItemData" => Some(Self::RollCageItemData),
            "RoofItemData" => Some(Self::RoofItemData),
            "SideSkirtsItemData" => Some(Self::SideSkirtsItemData),
            "SoundSystemItemData" => Some(Self::SoundSystemItemData),
            "SplitterItemData" => Some(Self::SplitterItemData),
            "SpoilerItemData" => Some(Self::SpoilerItemData),
            "StaticLiveryCustomizationItemData" => Some(Self::StaticLiveryCustomizationItemData),
            "SteeringTuningItemData" => Some(Self::SteeringTuningItemData),
            "StyleItemData" => Some(Self::StyleItemData),
            "SuspensionItemData" => Some(Self::SuspensionItemData),
            "SuspensionTuningItemData" => Some(Self::SuspensionTuningItemData),
            "SwaybarTuningItemData" => Some(Self::SwaybarTuningItemData),
            "TimedDiscountItemData" => Some(Self::TimedDiscountItemData),
            "TireCompositionItemData" => Some(Self::TireCompositionItemData),
            "TiresItemData" => Some(Self::TiresItemData),
            "TrunkLidItemData" => Some(Self::TrunkLidItemData),
            "WingMirrorsItemData" => Some(Self::WingMirrorsItemData),
            _ => None,
        }
    }
    pub fn layout(self) -> Layout {
        match self {
            Self::AirFilterItemData => Layout::Empty,
            Self::BodyKitItemData => Layout::Empty,
            Self::BrakeDiscsItemData => Layout::BrakeDiscs,
            Self::BrakesItemData => Layout::BrakeDiscs,
            Self::BumperItemData => Layout::Empty,
            Self::CalipersItemData => Layout::Empty,
            Self::CamShaftItemData => Layout::Empty,
            Self::CanardItemData => Layout::Empty,
            Self::CategoryUnlockControllerItemData => Layout::CategoryUnlockController,
            Self::ClutchItemData => Layout::Empty,
            Self::ControlArmTuningItemData => Layout::ControlArmTuning,
            Self::CurrencyItemData => Layout::Empty,
            Self::CylinderHeadsItemData => Layout::Empty,
            Self::DifferentialTuningItemData => Layout::DifferentialTuning,
            Self::DiffuserItemData => Layout::Empty,
            Self::DiscountItemData => Layout::Discount,
            Self::EcuItemData => Layout::Empty,
            Self::ElectricSystemItemData => Layout::Empty,
            Self::EngineBlockItemData => Layout::Empty,
            Self::EngineDisplacementItemData => Layout::Empty,
            Self::ExhaustItemData => Layout::Empty,
            Self::ExhaustManifoldItemData => Layout::Empty,
            Self::ExhaustSystemItemData => Layout::Empty,
            Self::FendersItemData => Layout::Empty,
            Self::ForcedInductionItemData => Layout::Empty,
            Self::FuelSystemItemData => Layout::Empty,
            Self::GearboxTuningItemData => Layout::GearboxTuning,
            Self::GearsItemData => Layout::Empty,
            Self::HandbrakeTuningItemData => Layout::HandbrakeTuning,
            Self::HoodItemData => Layout::Empty,
            Self::IgnitionItemData => Layout::Empty,
            Self::IntakeManifoldItemData => Layout::Empty,
            Self::LicensePlateBackgroundItemData => Layout::Empty,
            Self::LicensePlateFrameItemData => Layout::Empty,
            Self::LightsItemData => Layout::Empty,
            Self::LiveryCustomizationItemData => Layout::LiveryCustomization,
            Self::LiveryDecalSwatchPackItemData => Layout::Empty,
            Self::NosTuningItemData => Layout::NosTuning,
            Self::PersistantTuningItemData => Layout::PersistantTuning,
            Self::RaceVehicleItemData => Layout::RaceVehicle,
            Self::RadiatorItemData => Layout::Empty,
            Self::RimsItemData => Layout::Rims,
            Self::RollCageItemData => Layout::Empty,
            Self::RoofItemData => Layout::Empty,
            Self::SideSkirtsItemData => Layout::Empty,
            Self::SoundSystemItemData => Layout::Empty,
            Self::SplitterItemData => Layout::Empty,
            Self::SpoilerItemData => Layout::Spoiler,
            Self::StaticLiveryCustomizationItemData => Layout::Empty,
            Self::SteeringTuningItemData => Layout::SteeringTuning,
            Self::StyleItemData => Layout::Empty,
            Self::SuspensionItemData => Layout::Empty,
            Self::SuspensionTuningItemData => Layout::SuspensionTuning,
            Self::SwaybarTuningItemData => Layout::SwaybarTuning,
            Self::TimedDiscountItemData => Layout::TimedDiscount,
            Self::TireCompositionItemData => Layout::TireComposition,
            Self::TiresItemData => Layout::TireComposition,
            Self::TrunkLidItemData => Layout::Empty,
            Self::WingMirrorsItemData => Layout::Empty,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Layout {
    Empty,
    RaceVehicle,
    Spoiler,
    CategoryUnlockController,
    LiveryCustomization,
    NosTuning,
    PersistantTuning,
    SteeringTuning,
    SuspensionTuning,
    SwaybarTuning,
    TireComposition,
    BrakeDiscs,
    Discount,
    Rims,
    TimedDiscount,
    ControlArmTuning,
    DifferentialTuning,
    GearboxTuning,
    HandbrakeTuning,
}
#[derive(Clone, Eq, PartialEq)]
pub enum Derived {
    Empty,
    RaceVehicle {
        default_paint: [u32; 3],
        material: [u32; 3],
        window_tint: u32,
        default_license_plate_text: [u8; 8],
        max_hp: u32,
        max_torque: u32,
        zero_to_sixty: u32,
        zero_to_one_hundred: u32,
        top_speed_mph: u32,
        quarter_mile_mph: u32,
        quarter_mile_time: u32,
        vehicle_item_flags: u32,
    },
    Spoiler {
        downforce_setting: u32,
    },
    CategoryUnlockController {
        unlock_mask: u32,
    },
    LiveryCustomization {
        byte_vault_data_id: u32,
        library_preset_id: u32,
        dynamic_flags: u32,
    },
    NosTuning {
        nos_setting: u32,
    },
    PersistantTuning {
        abs_setting: u32,
        stability_control_setting: u32,
        traction_control_setting: u32,
        air_pressure_front_setting: u32,
        air_pressure_rear_setting: u32,
    },
    SteeringTuning {
        steer_rate_setting: u32,
        steer_range_setting: u32,
    },
    SuspensionTuning {
        spring_stiffness_front_setting: u32,
        spring_stiffness_rear_setting: u32,
        damping_front_setting: u32,
        damping_rear_setting: u32,
    },
    SwaybarTuning {
        anti_roll_bars_front_setting: u32,
        anti_roll_bars_rear_setting: u32,
    },
    TireComposition {
        tire_setting: u32,
    },
    BrakeDiscs {
        brake_strength_setting: u32,
        brake_bias_setting: u32,
    },
    Discount {
        discount_percent: u32,
    },
    Rims {
        rim_selection: u32,
        primary_paint: [u32; 3],
        primary_material: [u32; 3],
        secondary_paint: [u32; 3],
        secondary_material: [u32; 3],
    },
    TimedDiscount {
        discount_percent: u32,
        start_time: [u8; 24],
        end_time: [u8; 24],
        applicable_item_ids: Vec<u32>,
        applicable_item_tag_ids: Vec<u32>,
    },
    ControlArmTuning {
        caster_setting: u32,
        camber_front_setting: u32,
        camber_rear_setting: u32,
        ride_height_height_setting: u32,
        ride_height_rake_setting: u32,
        toe_front_setting: u32,
        toe_rear_setting: u32,
        track_width_front_setting: u32,
        track_width_rear_setting: u32,
    },
    DifferentialTuning {
        differential_setting: u32,
    },
    GearboxTuning {
        gearbox_setting: u32,
    },
    HandbrakeTuning {
        handbrake_strength_setting: u32,
    },
}
impl std::fmt::Debug for Derived {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Derived")
            .field("layout", &self.layout())
            .finish_non_exhaustive()
    }
}
impl Derived {
    pub fn layout(&self) -> Layout {
        match self {
            Self::Empty => Layout::Empty,
            Self::RaceVehicle { .. } => Layout::RaceVehicle,
            Self::Spoiler { .. } => Layout::Spoiler,
            Self::CategoryUnlockController { .. } => Layout::CategoryUnlockController,
            Self::LiveryCustomization { .. } => Layout::LiveryCustomization,
            Self::NosTuning { .. } => Layout::NosTuning,
            Self::PersistantTuning { .. } => Layout::PersistantTuning,
            Self::SteeringTuning { .. } => Layout::SteeringTuning,
            Self::SuspensionTuning { .. } => Layout::SuspensionTuning,
            Self::SwaybarTuning { .. } => Layout::SwaybarTuning,
            Self::TireComposition { .. } => Layout::TireComposition,
            Self::BrakeDiscs { .. } => Layout::BrakeDiscs,
            Self::Discount { .. } => Layout::Discount,
            Self::Rims { .. } => Layout::Rims,
            Self::TimedDiscount { .. } => Layout::TimedDiscount,
            Self::ControlArmTuning { .. } => Layout::ControlArmTuning,
            Self::DifferentialTuning { .. } => Layout::DifferentialTuning,
            Self::GearboxTuning { .. } => Layout::GearboxTuning,
            Self::HandbrakeTuning { .. } => Layout::HandbrakeTuning,
        }
    }
    pub(super) fn read(layout: Layout, r: &mut Reader<'_>) -> Result<Self, Error> {
        Ok(match layout {
            Layout::Empty => Self::Empty,
            Layout::RaceVehicle => Self::RaceVehicle {
                default_paint: [r.u32()?, r.u32()?, r.u32()?],
                material: [r.u32()?, r.u32()?, r.u32()?],
                window_tint: r.u32()?,
                default_license_plate_text: r.fixed()?,
                max_hp: r.u32()?,
                max_torque: r.u32()?,
                zero_to_sixty: r.u32()?,
                zero_to_one_hundred: r.u32()?,
                top_speed_mph: r.u32()?,
                quarter_mile_mph: r.u32()?,
                quarter_mile_time: r.u32()?,
                vehicle_item_flags: r.u32()?,
            },
            Layout::Spoiler => Self::Spoiler {
                downforce_setting: r.u32()?,
            },
            Layout::CategoryUnlockController => Self::CategoryUnlockController {
                unlock_mask: r.u32()?,
            },
            Layout::LiveryCustomization => Self::LiveryCustomization {
                byte_vault_data_id: r.u32()?,
                library_preset_id: r.u32()?,
                dynamic_flags: r.u32()?,
            },
            Layout::NosTuning => Self::NosTuning {
                nos_setting: r.u32()?,
            },
            Layout::PersistantTuning => Self::PersistantTuning {
                abs_setting: r.u32()?,
                stability_control_setting: r.u32()?,
                traction_control_setting: r.u32()?,
                air_pressure_front_setting: r.u32()?,
                air_pressure_rear_setting: r.u32()?,
            },
            Layout::SteeringTuning => Self::SteeringTuning {
                steer_rate_setting: r.u32()?,
                steer_range_setting: r.u32()?,
            },
            Layout::SuspensionTuning => Self::SuspensionTuning {
                spring_stiffness_front_setting: r.u32()?,
                spring_stiffness_rear_setting: r.u32()?,
                damping_front_setting: r.u32()?,
                damping_rear_setting: r.u32()?,
            },
            Layout::SwaybarTuning => Self::SwaybarTuning {
                anti_roll_bars_front_setting: r.u32()?,
                anti_roll_bars_rear_setting: r.u32()?,
            },
            Layout::TireComposition => Self::TireComposition {
                tire_setting: r.u32()?,
            },
            Layout::BrakeDiscs => Self::BrakeDiscs {
                brake_strength_setting: r.u32()?,
                brake_bias_setting: r.u32()?,
            },
            Layout::Discount => Self::Discount {
                discount_percent: r.u32()?,
            },
            Layout::Rims => Self::Rims {
                rim_selection: r.u32()?,
                primary_paint: [r.u32()?, r.u32()?, r.u32()?],
                primary_material: [r.u32()?, r.u32()?, r.u32()?],
                secondary_paint: [r.u32()?, r.u32()?, r.u32()?],
                secondary_material: [r.u32()?, r.u32()?, r.u32()?],
            },
            Layout::TimedDiscount => Self::TimedDiscount {
                discount_percent: r.u32()?,
                start_time: r.fixed()?,
                end_time: r.fixed()?,
                applicable_item_ids: r.words()?,
                applicable_item_tag_ids: r.words()?,
            },
            Layout::ControlArmTuning => Self::ControlArmTuning {
                caster_setting: r.u32()?,
                camber_front_setting: r.u32()?,
                camber_rear_setting: r.u32()?,
                ride_height_height_setting: r.u32()?,
                ride_height_rake_setting: r.u32()?,
                toe_front_setting: r.u32()?,
                toe_rear_setting: r.u32()?,
                track_width_front_setting: r.u32()?,
                track_width_rear_setting: r.u32()?,
            },
            Layout::DifferentialTuning => Self::DifferentialTuning {
                differential_setting: r.u32()?,
            },
            Layout::GearboxTuning => Self::GearboxTuning {
                gearbox_setting: r.u32()?,
            },
            Layout::HandbrakeTuning => Self::HandbrakeTuning {
                handbrake_strength_setting: r.u32()?,
            },
        })
    }
    pub(super) fn write(&self, w: &mut Writer) -> Result<(), Error> {
        match self {
            Self::Empty => {}
            Self::RaceVehicle {
                default_paint,
                material,
                window_tint,
                default_license_plate_text,
                max_hp,
                max_torque,
                zero_to_sixty,
                zero_to_one_hundred,
                top_speed_mph,
                quarter_mile_mph,
                quarter_mile_time,
                vehicle_item_flags,
            } => {
                for word in default_paint {
                    w.u32(*word)?;
                }
                for word in material {
                    w.u32(*word)?;
                }
                w.u32(*window_tint)?;
                w.bytes(default_license_plate_text)?;
                w.u32(*max_hp)?;
                w.u32(*max_torque)?;
                w.u32(*zero_to_sixty)?;
                w.u32(*zero_to_one_hundred)?;
                w.u32(*top_speed_mph)?;
                w.u32(*quarter_mile_mph)?;
                w.u32(*quarter_mile_time)?;
                w.u32(*vehicle_item_flags)?;
            }
            Self::Spoiler { downforce_setting } => {
                w.u32(*downforce_setting)?;
            }
            Self::CategoryUnlockController { unlock_mask } => {
                w.u32(*unlock_mask)?;
            }
            Self::LiveryCustomization {
                byte_vault_data_id,
                library_preset_id,
                dynamic_flags,
            } => {
                w.u32(*byte_vault_data_id)?;
                w.u32(*library_preset_id)?;
                w.u32(*dynamic_flags)?;
            }
            Self::NosTuning { nos_setting } => {
                w.u32(*nos_setting)?;
            }
            Self::PersistantTuning {
                abs_setting,
                stability_control_setting,
                traction_control_setting,
                air_pressure_front_setting,
                air_pressure_rear_setting,
            } => {
                w.u32(*abs_setting)?;
                w.u32(*stability_control_setting)?;
                w.u32(*traction_control_setting)?;
                w.u32(*air_pressure_front_setting)?;
                w.u32(*air_pressure_rear_setting)?;
            }
            Self::SteeringTuning {
                steer_rate_setting,
                steer_range_setting,
            } => {
                w.u32(*steer_rate_setting)?;
                w.u32(*steer_range_setting)?;
            }
            Self::SuspensionTuning {
                spring_stiffness_front_setting,
                spring_stiffness_rear_setting,
                damping_front_setting,
                damping_rear_setting,
            } => {
                w.u32(*spring_stiffness_front_setting)?;
                w.u32(*spring_stiffness_rear_setting)?;
                w.u32(*damping_front_setting)?;
                w.u32(*damping_rear_setting)?;
            }
            Self::SwaybarTuning {
                anti_roll_bars_front_setting,
                anti_roll_bars_rear_setting,
            } => {
                w.u32(*anti_roll_bars_front_setting)?;
                w.u32(*anti_roll_bars_rear_setting)?;
            }
            Self::TireComposition { tire_setting } => {
                w.u32(*tire_setting)?;
            }
            Self::BrakeDiscs {
                brake_strength_setting,
                brake_bias_setting,
            } => {
                w.u32(*brake_strength_setting)?;
                w.u32(*brake_bias_setting)?;
            }
            Self::Discount { discount_percent } => {
                w.u32(*discount_percent)?;
            }
            Self::Rims {
                rim_selection,
                primary_paint,
                primary_material,
                secondary_paint,
                secondary_material,
            } => {
                w.u32(*rim_selection)?;
                for word in primary_paint {
                    w.u32(*word)?;
                }
                for word in primary_material {
                    w.u32(*word)?;
                }
                for word in secondary_paint {
                    w.u32(*word)?;
                }
                for word in secondary_material {
                    w.u32(*word)?;
                }
            }
            Self::TimedDiscount {
                discount_percent,
                start_time,
                end_time,
                applicable_item_ids,
                applicable_item_tag_ids,
            } => {
                w.u32(*discount_percent)?;
                w.bytes(start_time)?;
                w.bytes(end_time)?;
                w.words(applicable_item_ids)?;
                w.words(applicable_item_tag_ids)?;
            }
            Self::ControlArmTuning {
                caster_setting,
                camber_front_setting,
                camber_rear_setting,
                ride_height_height_setting,
                ride_height_rake_setting,
                toe_front_setting,
                toe_rear_setting,
                track_width_front_setting,
                track_width_rear_setting,
            } => {
                w.u32(*caster_setting)?;
                w.u32(*camber_front_setting)?;
                w.u32(*camber_rear_setting)?;
                w.u32(*ride_height_height_setting)?;
                w.u32(*ride_height_rake_setting)?;
                w.u32(*toe_front_setting)?;
                w.u32(*toe_rear_setting)?;
                w.u32(*track_width_front_setting)?;
                w.u32(*track_width_rear_setting)?;
            }
            Self::DifferentialTuning {
                differential_setting,
            } => {
                w.u32(*differential_setting)?;
            }
            Self::GearboxTuning { gearbox_setting } => {
                w.u32(*gearbox_setting)?;
            }
            Self::HandbrakeTuning {
                handbrake_strength_setting,
            } => {
                w.u32(*handbrake_strength_setting)?;
            }
        }
        Ok(())
    }
}
